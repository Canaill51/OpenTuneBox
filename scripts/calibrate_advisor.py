#!/usr/bin/env python3
"""Recalcule les statistiques du corpus utilisées pour calibrer les seuils de l'assistant.

Usage : calibrate_advisor.py NB_FICHIERS SORTIE.json [-j PROCESSUS]  (dossier de logs : variable OTB_LOGS ;
        défaut : tous les cœurs, -j 1 = un fichier à la fois)
        calibrate_advisor.py --resume SORTIE.json      (p10 / p50 / p90 par taille, voir docs/assistant.md)

Les sessions sont reclassées par taille avec les limites actuelles (fréquence moteur et cellules). Les
retards des filtres ne comptent que pour les vols avec mouvements (delay.MOVING_SETPOINT_DPS). Tout le
corpus compte : pas de corpus de référence (états « regle » / « origine » invérifiables, docs/assistant.md).
"""
import sys, json, os, random
from functools import partial
import numpy as np
from opentunebox.log.flightlog import load
from opentunebox.analysis.period import auto_period
from opentunebox.analysis.spectral import spectrum
from opentunebox.analysis.step import step_response
from opentunebox.analysis.delay import MOVING_SETPOINT_DPS, compute_delays, stick_p95_dps, xcorr_delay_ms
from opentunebox.analysis.motors import harmonic_prominence, motor_noise
from opentunebox.advisor.profiles import PROFILES, detect_profile, profile_key
from opentunebox.advisor.rules import band_peak_ptb_db, frame_resonance
from opentunebox.analysis.motors import hover_motor_hz
from opentunebox.analysis.stats import flight_stats
from corpus_labels import drone_id, log_files, logs_root, map_files

def summary(path):
    """Affiche, par taille puis pour tout le corpus (TOUS) : p10 / p50 / p90 par session (roll et pitch
    regroupés) et p10 / p90 avec une voix par drone (médiane de ses sessions ; drone = nom, sinon dossier)."""
    rows=json.load(open(path))
    for r in rows:
        if r.get("hover_hz"): r["profile"]=profile_key(r["hover_hz"],r.get("cells"))  # limites actuelles
        r["drone"]=drone_id(r.get("craft"),r["file"])
        # mesures dérivées, telles que les règles les utilisent
        if "peak_0" in r and "peak_1" in r: r["peakdiff"]=abs(r["peak_1"]-r["peak_0"])
        if "imean_0" in r: r["imean_abs"]=max(abs(r["imean_0"]),abs(r["imean_1"]))
        for a in range(2):
            if f"mnoise_{a}" in r: r[f"mleft_{a}"]=r[f"mnoise_{a}"][0]
            for k,n in (("hprom","prom"),("hleft","left")):  # par harmonique : prom1…3, left1…3
                for h,v in enumerate(r.get(f"{k}_{a}") or []): r[f"{n}{h+1}_{a}"]=v
        if r.get("res_db") is not None and not r.get("res_near"): r["res_free_db"]=r["res_db"]
        if r.get("sp95",1e9)<MOVING_SETPOINT_DPS:  # vol sans mouvements : retards des filtres non représentatifs
            for k in [k for k in r if k.startswith(("gdelay","ddelay"))]: del r[k]
    keys=sorted({k.rsplit("_",1)[0] if k[-2:] in ("_0","_1") else k for r in rows for k in r}
                - {"file","sess","profile","craft","drone","res_near","mnoise","hprom","hleft","rpmw","imean","cells","sliders","ff","dgain","pgain"})
    def vals(sub,k):
        return [x for r in sub for n in (k,k+"_0",k+"_1") for x in [r.get(n)]
                if isinstance(x,(int,float)) and not isinstance(x,bool) and np.isfinite(x)]
    ax=[(r,a) for r in rows for a in range(2) if f"measured_{a}" in r]
    if ax:  # boucle ouverte (analysis/loop.py) : part des axes mesurés, croisements trouvés, pics au bord
        meas=[(r,a) for r,a in ax if r[f"measured_{a}"]]
        fc=sum(np.isfinite(r[f"fc_{a}"]) for r,a in meas); edge=sum(r[f"smaxedge_{a}"] for r,a in meas)
        print(f"Boucle ouverte : {len(meas)}/{len(ax)} axes mesurés ({100*len(meas)/len(ax):.0f} %), croisement trouvé"
              f" {100*fc/max(1,len(meas)):.0f} %, pic de sensibilité au bord {100*edge/max(1,len(meas)):.0f} %")
    groups=[(key,[r for r in rows if key=="TOUS" or r["profile"]==key]) for key in [*PROFILES,"TOUS"]]
    for key,sub in groups:
        print(f"\n== {key} : {len(sub)} sessions, {len({r['drone'] for r in sub})} drones"
              "   (par session p10 / p50 / p90 | une voix par drone p10 / p90)")
        for k in keys:
            v=np.array(vals(sub,k))
            if not v.size: continue
            by={}
            for r in sub:
                x=vals([r],k)
                if x: by.setdefault(r["drone"],[]).extend(x)
            d=np.array([np.median(x) for x in by.values()])
            print(f"{k:12s} n={v.size:4d}  "+" / ".join(f"{x:.2f}" for x in np.percentile(v,[10,50,90]))
                  +f"  | {len(d):3d} drones  "+" / ".join(f"{x:.2f}" for x in np.percentile(d,[10,90])))

def measure_file(f, root):
    """Mesures de chaque session analysable d'un log (au moins 8 s, période analysée d'au moins 5 s)."""
    out=[]
    for lg in load(f):
        if lg.duration < 8: continue
        t0,t1=auto_period(lg); sel=lg.slice_time(t0,t1)
        if sel.stop-sel.start < lg.fs*5: continue
        prof,_=detect_profile(lg)
        r={"file":os.path.relpath(f,root),"sess":lg.session,"profile":prof.key,"fs":lg.fs,
           "craft":lg.settings.craft_name,"hover_hz":hover_motor_hz(lg),"cells":lg.cells,
           "corrupt":lg.stats.get("corrupt_frames",0),"sliders":lg.settings.sliders()}
        nyq=lg.fs/2
        def band(x,lo,hi):
            s=spectrum(x,lg.fs,resolution_hz=2); m=(s.freqs>=lo)&(s.freqs<=min(hi,nyq*0.95)); return float(np.mean(s.values[m]))
        r["rpmq"]=lg.settings.rpm_q; r["rpmh"]=lg.settings.rpm_harmonics; r["rpmw"]=lg.settings.rpm_weights
        for a in range(2):
            r[f"gyro_{a}"]=band(lg.gyro[a,sel],80,500)
            r[f"gptb_{a}"]=band_peak_ptb_db(lg.gyro[a,sel],lg.fs,80,500)  # échelle PIDtoolbox
            if np.any(lg.d_term[a]): r[f"dptb_{a}"]=band_peak_ptb_db(lg.d_term[a,sel],lg.fs,80,500)
            if lg.gyro_unfilt is not None:
                r[f"gdelayx_{a}"]=xcorr_delay_ms(lg.gyro_unfilt[a,sel],lg.gyro[a,sel],lg.fs,30.0)
                if lg.motor_hz is not None:  # hauteur des harmoniques au-dessus du bruit voisin, avant / après filtres
                    r[f"hprom_{a}"]=harmonic_prominence(lg.gyro_unfilt[a,sel],lg.motor_hz[:,sel],lg.fs).tolist()
                    r[f"hleft_{a}"]=harmonic_prominence(lg.gyro[a,sel],lg.motor_hz[:,sel],lg.fs).tolist()
            if lg.gyro_unfilt is not None: r[f"gyrou_{a}"]=band(lg.gyro_unfilt[a,sel],80,500)
            if np.any(lg.d_term[a]): r[f"dterm_{a}"]=band(lg.d_term[a,sel],80,500)
            r[f"lowband_{a}"]=band(lg.gyro[a,sel],20,80)
            st=step_response(lg.setpoint[a,sel],lg.gyro[a,sel],lg.fs,terms=lg.pid_terms(a,sel))
            if st.n>=20:
                lp=st.loop
                r[f"peak_{a}"]=st.metrics.peak; r[f"lat_{a}"]=st.metrics.latency_ms
                r[f"nwin_{a}"]=st.n; r[f"ripple_{a}"]=st.metrics.ripple  # vagues : mesure à vide du Diagnostic du réglage
                r[f"waves_{a}"]=st.metrics.waves; r[f"wavesr_{a}"]=st.metrics.waves_ratio
                pid=lg.settings.pids[a]; r[f"ff_{a}"]=pid.F; r[f"dgain_{a}"]=pid.D; r[f"pgain_{a}"]=pid.P  # pic selon le feedforward
                r[f"smax_{a}"]=lp.sensitivity_peak_db; r[f"pm_{a}"]=lp.phase_margin_deg  # comme les règles
                r[f"measured_{a}"]=lp.measured; r[f"smaxedge_{a}"]=lp.sensitivity_peak_at_edge
                r[f"fc_{a}"]=lp.crossover_hz; r[f"gm_{a}"]=lp.gain_margin_db; r[f"dratio_{a}"]=lp.d_ratio
                r[f"bandtop_{a}"]=lp.band_hz[1]; r[f"track_{a}"]=lp.tracking_peak_db
                if lp.measured:  # mêmes mesures par l'ancienne formule T/(1 − T), pour comparaison
                    old=step_response(lg.setpoint[a,sel],lg.gyro[a,sel],lg.fs).loop
                    r[f"smaxold_{a}"]=old.sensitivity_peak_db; r[f"pmold_{a}"]=old.phase_margin_deg
                    r[f"fcold_{a}"]=old.crossover_hz
            if lg.motor_hz is not None:
                mn=motor_noise(lg.gyro[a,sel],lg.motor_hz[:,sel],lg.fs); r[f"mnoise_{a}"]=mn.mean_db.tolist()
        d=compute_delays(lg,sel); r["gdelay"]=d.gyro_ms; r["ddelay"]=d.dterm_ms; r["sp95"]=stick_p95_dps(lg,sel)
        fst=flight_stats(lg,sel); r["sat"]=fst.motor_saturation_pct; r["imb"]=fst.motor_imbalance_pct
        for a in range(2): r[f"imean_{a}"]=float(np.mean(lg.i_term[a,sel]))
        res=frame_resonance(lg,sel)
        if res is not None: r["res_hz"],r["res_db"],r["res_near"]=res
        out.append(r)
    return out

if __name__=="__main__":
    if sys.argv[1]=="--resume":
        summary(sys.argv[2]); sys.exit()
    jobs=int(sys.argv[sys.argv.index("-j")+1]) if "-j" in sys.argv else None
    root=logs_root()
    files=log_files(root)  # sans doublons ni __MACOSX
    random.seed(0); random.shuffle(files)
    out=[r for _,rows in map_files(partial(measure_file,root=root),files[:int(sys.argv[1])],jobs,root) for r in rows]
    json.dump(out,open(sys.argv[2],"w"))
    print(len(out),"sessions")
