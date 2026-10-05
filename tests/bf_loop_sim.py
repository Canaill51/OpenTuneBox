"""Simulation d'une boucle de régulation Betaflight (un axe), pour tester la mesure de la boucle ouverte.

Tout tourne à la cadence de la boucle PID, dans l'ordre du firmware (`flight/pid.c`) :
- gyro filtré = passe-bas PT1 du gyro brut (vitesse du drone + bruit capteur) : c'est le gyro loggé ;
- P = Kp·e et I += Ki·e·dt sur l'erreur e = consigne − gyro filtré (P, I : dans `pid.c`) ;
- D = −Kd·d(gyro filtré par les filtres du D)/dt (D sur le gyro) ; F = Kf·d(consigne)/dt lissé ;
- u = P + I + D + F, plus une perturbation (vent, prop wash) → retard → moteur (PT1) → accélération ;
- le log prend une trame sur `log_div`, valeurs arrondies à l'entier comme `lrintf`.

Gains en unités de l'en-tête (P, I, D, F) avec les échelles de `pid.c` (PTERM_SCALE…). La boucle ouverte
exacte (coupée à l'entrée du drone) est L = G·(Ce + Cd), calculée sur le même modèle discret.
"""

from __future__ import annotations

from dataclasses import dataclass

import numpy as np
from numba import njit
from scipy import signal

PTERM_SCALE, ITERM_SCALE, DTERM_SCALE, FF_SCALE = 0.032029, 0.244381, 0.000529, 0.013754  # pid.h ; Kf = FF_SCALE × F/100 (pid_init.c)


@dataclass
class Sim:
    fs: float  # cadence du log
    r: np.ndarray  # consigne loggée (°/s)
    y: np.ndarray  # gyro loggé (°/s)
    p: np.ndarray
    i: np.ndarray
    d: np.ndarray
    f: np.ndarray
    params: dict

    def true_parts(self, freqs, **override):
        """(G, Ce, Cd, Cf) exacts aux fréquences `freqs` (Hz) ; `override` change un paramètre (ex. D)."""
        return _parts(np.asarray(freqs, float), **{**self.params, **override})

    def true_L(self, freqs, **override):
        G, Ce, Cd, _ = self.true_parts(freqs, **override)
        return G * (Ce + Cd)


def _pt1_alpha(f_hz, fp):
    k = 2 * np.pi * f_hz / fp
    return k / (k + 1)


def _pt1(alpha, z1):
    return alpha / (1 - (1 - alpha) * z1)


def _parts(f, fp, P, I, D, F, body_gain, motor_tau, delay, gyro_lpf, dterm_lpf, ff_lpf, **_):
    z1 = np.exp(-2j * np.pi * f / fp)
    dt = 1 / fp
    fg = _pt1(_pt1_alpha(gyro_lpf, fp), z1)
    fd = np.ones_like(z1)
    for hz in dterm_lpf:
        fd = fd * _pt1(_pt1_alpha(hz, fp), z1)
    motor = _pt1(dt / (motor_tau + dt), z1)
    body = body_gain * dt * z1 / (1 - z1)
    G = fg * body * motor * z1**delay
    Ce = P * PTERM_SCALE + I * ITERM_SCALE * dt / (1 - z1)
    Cd = D * DTERM_SCALE * fp * (1 - z1) * fd
    Cf = F * FF_SCALE * 0.01 * fp * (1 - z1) * _pt1(_pt1_alpha(ff_lpf, fp), z1)
    return G, Ce, Cd, Cf


@njit(cache=True)
def _run(r, dist, noise, kd_mult, fp, kp, ki, kd, kf, body_gain, motor_tau, delay, ag, ad1, ad2, aff):
    n = r.size
    dt = 1.0 / fp
    am = dt / (motor_tau + dt)
    out = np.zeros((5, n))  # gyro filtré, P, I, D, F
    body = 0.0
    yg = 0.0
    yd1 = 0.0
    yd2 = 0.0
    yd_prev = 0.0
    iterm = 0.0
    ff = 0.0
    motor = 0.0
    buf = np.zeros(delay + 1)
    for k in range(n):
        yg += ag * (body + noise[k] - yg)
        e = r[k] - yg
        p = kp * e
        iterm += ki * e * dt
        yd1 += ad1 * (yg - yd1)
        yd2 += ad2 * (yd1 - yd2)
        dterm = -kd * kd_mult[k] * (yd2 - yd_prev) * fp
        yd_prev = yd2
        dr = (r[k] - r[k - 1]) * fp if k > 0 else 0.0
        ff += aff * (kf * dr - ff)
        u = p + iterm + dterm + ff
        for j in range(delay, 0, -1):
            buf[j] = buf[j - 1]
        buf[0] = u + dist[k]
        motor += am * (buf[delay] - motor)
        out[0, k] = yg
        out[1, k] = p
        out[2, k] = iterm
        out[3, k] = dterm
        out[4, k] = ff
        body += body_gain * motor * dt
    return out


def stick_setpoint(n, fp, rng, rc_hz=250.0, max_rate=600.0):
    """Mouvements de manche : paliers aléatoires lissés, échantillonnés comme une liaison radio
    (`rc_hz`, rc_smoothing ≈ 40 Hz) ; la consigne garde ainsi un peu d'énergie au-delà de 50 Hz."""
    sp = np.zeros(n)
    i = 0
    while i < n:
        d = int(rng.uniform(0.05, 0.5) * fp)
        sp[i:i + d] = rng.choice([0, 1, -1, 0.3, -0.3]) * rng.uniform(50, max_rate)
        i += d
    sp = signal.lfilter(*signal.butter(2, 15 / (fp / 2)), sp)
    hold = max(1, int(round(fp / rc_hz)))
    sp = np.repeat(sp[::hold], hold)[:n]  # paliers de la liaison radio
    return signal.lfilter(*signal.butter(1, 40 / (fp / 2)), sp)


def simulate(seconds=60.0, fp=4000.0, log_div=2, P=45, I=80, D=35, F=120, body_gain=120.0,
             motor_tau=0.020, delay=4, gyro_lpf=150.0, dterm_lpf=(90.0, 180.0), ff_lpf=30.0,
             dist=40.0, noise=4.0, d_boost=0.0, seed=0) -> Sim:
    """Une session simulée. `dist` : écart-type de la perturbation à l'entrée du drone (unités PID) ;
    `noise` : bruit du gyro (°/s) ; `d_boost` : D variable, Kd × (1 + d_boost·|vitesse du manche|)
    comme D max (le vrai L est alors celui du D moyen)."""
    rng = np.random.default_rng(seed)
    n = int(seconds * fp)
    r = stick_setpoint(n, fp, rng)
    w = rng.normal(0, 1, n)
    d_in = signal.lfilter(*signal.butter(1, 30 / (fp / 2)), w)
    d_in = dist * (d_in / d_in.std() + 0.3 * rng.normal(0, 1, n))
    t = np.arange(n) / fp
    sensor = noise * rng.normal(0, 1, n) + 3 * noise * np.sin(2 * np.pi * 260 * t)
    speed = np.abs(np.gradient(r) * fp)
    kd_mult = 1 + d_boost * np.clip(speed / 5000.0, 0, 1)
    out = _run(r, d_in, sensor, kd_mult, fp, P * PTERM_SCALE, I * ITERM_SCALE, D * DTERM_SCALE, F * FF_SCALE * 0.01,
               body_gain, motor_tau, int(delay), _pt1_alpha(gyro_lpf, fp), _pt1_alpha(dterm_lpf[0], fp),
               _pt1_alpha(dterm_lpf[1], fp), _pt1_alpha(ff_lpf, fp))
    sl = slice(0, n, log_div)
    params = dict(fp=fp, P=P, I=I, D=D * float(np.mean(kd_mult)), F=F, body_gain=body_gain, motor_tau=motor_tau,
                  delay=delay, gyro_lpf=gyro_lpf, dterm_lpf=dterm_lpf, ff_lpf=ff_lpf)
    rnd = np.rint
    return Sim(fp / log_div, rnd(r[sl]), rnd(out[0, sl]), rnd(out[1, sl]), rnd(out[2, sl]), rnd(out[3, sl]),
               rnd(out[4, sl]), params)
