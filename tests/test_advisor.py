"""Tests de l'assistant de réglage et des fenêtres associées (sur logs réels)."""

import os

import pytest

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")

from corpus_paths import LOGS  # noqa: E402
pytestmark = [pytest.mark.corpus, pytest.mark.skipif(not LOGS.exists(), reason="corpus absent")]


def _load(rel):
    from opentunebox.log.flightlog import load

    return load(LOGS / rel)[0]


def test_profiles_detected_by_motor_speed():
    from opentunebox.advisor.profiles import detect_profile

    assert detect_profile(_load("Cedric/Air65/1.Stock.bbl"))[0].key == "whoop"
    assert detect_profile(_load("Cedric/JeNo 5/FINAL.bbl"))[0].key == "five"
    # Meteor 75 (≈ 355 Hz en vol) : whoop depuis la limite à 345 Hz
    assert detect_profile(_load("Cedric/Meteor 75 Pro O4 HD/03112025/D.bbl"))[0].key == "whoop"
    # JeNo 3,5" en 4S : tourne à ~235 Hz comme un 5", reconnu par le nombre de cellules
    assert detect_profile(_load("Cedric/J3.5/Emax Avan/24k-Emax.bbl"))[0].key == "micro"


def test_profile_thresholds():
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES, profile_key
    from opentunebox.advisor.rules import analyze

    assert [profile_key(h) for h in (480, 350, 300, 220, 165, 140, 100)] == \
        ["whoop", "whoop", "micro", "five", "five", "seven", "large"]
    # cellules : 1S = whoop ; petit drone en 2–4S au-dessus de 200 Hz ; rien pour le 6S (recouvrement)
    assert [profile_key(h, c) for h, c in ((306, 1), (237, 4), (195, 4), (255, 6), (242, 2))] == \
        ["whoop", "micro", "five", "five", "micro"]
    lg = _load("Cedric/JeNo 5/FINAL.bbl")
    titles = lambda rep: {f.title for f in rep.findings}  # noqa: E731
    # un seuil mécanique nul déclenche le constat : la règle lit bien le profil
    strict = replace(PROFILES["five"], imbalance_high_pct=0.0)
    assert "Moteurs déséquilibrés" in titles(analyze(lg, profile=strict))
    assert "Seuils provisoires pour cette taille" in titles(analyze(lg, profile=PROFILES["micro"]))
    assert "Seuils provisoires pour cette taille" not in titles(analyze(lg, profile=PROFILES["five"]))


def test_overshoot_detected_on_low_damping():
    from opentunebox.advisor.rules import ATTENTION, CONSEIL, analyze
    from opentunebox.analysis.period import auto_period

    lg = _load("1. PTB/BlackSummer/Damping 0.8.BFL")  # pic roll 1,25 sans feedforward
    rep = analyze(lg, lg.slice_time(*auto_period(lg)))
    titles = [f.title for f in rep.findings if f.severity in (CONSEIL, ATTENTION)]
    assert any("dépassement" in t and "roll" in t for t in titles)
    # chaque constat explique sa mesure
    assert all(f.detail for f in rep.findings)
    # première valeur de Damping sans dépassement visible (pic 1,03) : rien à signaler
    lg = _load("1. PTB/BlackSummer/Damping 1.2.BFL")
    rep = analyze(lg, lg.slice_time(*auto_period(lg)))
    assert not any("dépassement" in f.title for f in rep.findings)


def test_dterm_noise_on_ptb_scale():
    from opentunebox.advisor.rules import DTERM_PTB_MAX_DB, analyze

    # Kris : terme D au-dessus du repère −10 dB de PIDtoolbox, gyro filtré au-dessus de −30 dB
    rep = analyze(_load("1. PTB/Kris/BTFL_BLACKBOX_LOG_TP_20260914_181415_F722_AIO.bbl"))
    assert rep.measures["dterm_peak_ptb_db"] > DTERM_PTB_MAX_DB
    d = next(f for f in rep.findings if f.title == "Terme D bruité")
    assert "PIDtoolbox" in d.detail
    g = next(f for f in rep.findings if "gyro filtré" in f.title.lower())
    assert "PIDtoolbox" in g.detail


def test_filter_delay_targets_and_rpm_q():
    from dataclasses import replace

    from opentunebox.advisor.profiles import PROFILES, filter_delay_target_ms
    from opentunebox.advisor.rules import RPM_Q_MAX, analyze
    from opentunebox.analysis.period import auto_period

    # graphique de Brian White : 1 ms à 2", 4 ms (gyro) et 3 ms (D) à 14"
    assert filter_delay_target_ms(5) == (1.75, 1.5)
    assert filter_delay_target_ms(14) == (4.0, 3.0)
    assert filter_delay_target_ms(None) is None and filter_delay_target_ms(1.6) is None
    # repère de retard nul : le conseil apparaît, avec l'objectif et le Q RPM plus haut si Q < 750
    lg = _load("1. PTB/Fred28/Stock.BFL")
    assert lg.settings.rpm_q < RPM_Q_MAX
    rep = analyze(lg, lg.slice_time(*auto_period(lg)), replace(PROFILES["five"], gyro_delay_max_ms=0.0))
    f = next(f for f in rep.findings if f.title == "Filtrage gyro peut-être trop prudent")
    assert "Brian White" in f.detail
    assert any("rpm_filter_q" in a for a in f.actions)


def test_rpm_weights_rule():
    from opentunebox.advisor.rules import analyze
    from opentunebox.analysis.period import auto_period

    # Meteor 75 d'origine : 3e harmonique invisible avant filtrage, poids 100 → poids réduit proposé, les
    # autres gardés
    lg = _load("Cedric/Meteor 75 Pro O4 HD/48k Base/stock.bbl")
    rep = analyze(lg, lg.slice_time(*auto_period(lg)))
    f = next(f for f in rep.findings if f.title == "Filtre RPM plus fort que nécessaire")
    assert "3e harmonique" in f.detail
    assert f.actions[0].startswith("rpm_filter_weights = 100, 100, 50")


def test_dyn_notch_rules():
    from opentunebox.advisor.rules import analyze
    from opentunebox.analysis.period import auto_period

    def report(rel):
        lg = _load(rel)
        return analyze(lg, lg.slice_time(*auto_period(lg)))

    # Kris : raie fixe à 97 Hz sous la plage du notch dynamique (80–350 Hz, marge de 20 Hz) → plage élargie,
    # et pas de relèvement de dyn_notch_min_hz proposé en même temps
    rep = report("1. PTB/Kris/13. Hover.bbl")
    f = next(f for f in rep.findings if f.title.startswith("Raie fixe"))
    assert any("dyn_notch_min_hz = 70" in a for a in f.actions)
    assert not any("dyn_notch_min_hz 80 →" in a for g in rep.findings for a in g.actions)
    # Thalhammer (7") : résonance à 37 Hz, sous 60 Hz → dyn_notch_min_hz jusqu'à la limite du firmware (20 Hz)
    from opentunebox.log.flightlog import load

    found = [f for lg in load(LOGS / "1. PTB/Thalhammer/Hover Test 48kHz.bbl")
             for f in analyze(lg, lg.slice_time(*auto_period(lg))).findings if f.title == "Raie fixe vers 37 Hz"]
    assert found and any("dyn_notch_min_hz = 20," in a for a in found[0].actions)
    # KL (log à 2 kHz) : moins de pics à suivre que de notchs réglés → nombre réduit (info)
    rep = report("Cedric/KL/f3.bbl")
    f = next(f for f in rep.findings if f.title == "Notch dynamique : moins de notchs suffiraient")
    assert f.actions[0].startswith("dyn_notch_count = 2 (actuellement 3)")
    # JeNo 5 « PD » enregistré à 1 kHz : spectre replié, pas d'allègement proposé
    assert not any(f.title.startswith("Notch dynamique") for f in report("Cedric/JeNo 5/PD.bbl").findings)


def test_lowpass_rules():
    from opentunebox.advisor.rules import ATTENTION, CONSEIL, INFO, analyze
    from opentunebox.analysis.period import auto_period

    def findings(rel):
        lg = _load(rel)
        return {f.title: f for f in analyze(lg, lg.slice_time(*auto_period(lg))).findings}

    # réglages : LPF2 du gyro coupé à 4 kHz de PID pour 8 kHz de gyro ; D d'ordre 1 ; LPF1 du D statique
    f = findings("1. PTB/StraylightFPV/KM/Filtering/011326 KM HOVER.bbl")["LPF2 du gyro coupé : risque de repliement"]
    assert f.severity == CONSEIL
    assert findings("1. PTB/StraylightFPV/KM/PID/011326 KM PDBAL 1.4.bbl")["Filtrage du D insuffisant"].severity == ATTENTION
    assert findings("Pelafoin/btfl_PUNCH_1-modif2.bbl")["LPF1 du D en mode statique"].severity == INFO
    # simulateur : KL d'origine, LPF1 du gyro superflu avec le filtre RPM
    f = findings("Cedric/KL/stock.bbl")["Le LPF1 du gyro peut être coupé"]
    assert "ms de retard en moins" in f.detail and f.actions[0].startswith("gyro_lpf1_static_hz = 0")
    # retard du D au-dessus du repère, D propre : relèvement proposé avec la prévision
    f = findings("1. PTB/Fitz/btfl_004.bbl")["Filtrage du D peut-être trop fort"]
    assert "le simulateur prévoit" in f.detail and f.actions[0] == "Curseur « filtre D » : +0,1."
    # vol stationnaire : retard non représentatif, aucun conseil fondé sur le retard
    hover = findings("1. PTB/BlackSummer/Hover_24kHz.bfl")
    assert hover["Retard des filtres peu représentatif"].severity == INFO
    assert not {"Filtrage du D peut-être trop fort", "Filtrage gyro peut-être trop prudent",
                "Le LPF1 du gyro peut être coupé"} & set(hover)


def test_filter_strategy_needs_two_stages():
    from opentunebox.advisor.rules import CONSEIL, Finding, filter_strategy

    rate = Finding("Filtres", CONSEIL, "Fréquence d'enregistrement trop basse", "", ["blackbox_sample_rate = 1/2"])
    rpm = Finding("Filtres", CONSEIL, "Poids du filtre RPM trop bas", "", ["rpm_filter_weights = 100,100,100"])
    assert filter_strategy([rate]) is None and filter_strategy([rpm]) is None  # le constat seul suffit
    s = filter_strategy([rpm, rate])
    assert s.actions[0].startswith("1. Enregistrement") and s.actions[1].startswith("2. Filtre RPM")


def test_hover_noise_and_filter_strategy():
    from opentunebox.advisor.rules import analyze
    from opentunebox.analysis.period import auto_period

    def report(rel, session=1):
        from opentunebox.log.flightlog import load

        lg = next(x for x in load(LOGS / rel) if x.session == session)
        return analyze(lg, lg.slice_time(*auto_period(lg))).findings

    # 7" de Thalhammer en stationnaire (3e session) : 3e harmonique hors du filtre RPM (2 harmoniques) ;
    # stratégie en tête, bruit à corriger d'abord, puis le filtre RPM
    rep = report("1. PTB/Thalhammer/Hover Test 48kHz.bbl", session=3)
    f = {x.title: x for x in rep}
    assert "Vol stationnaire : bruit identifié" in f
    assert f["Harmonique moteur hors du filtre RPM"].actions[0].startswith("rpm_filter_harmonics = 3")
    assert rep[0].title == "Stratégie de filtrage" and rep[0].actions[0].startswith("1. À corriger d'abord")
    assert any(a.split(" — ")[0].endswith("Filtre RPM") for a in rep[0].actions)
    # J3.5 en stationnaire : raie à 67 Hz que rien ne filtre, avec la mise en garde sous 100 Hz
    f = {x.title: x for x in report("Cedric/J3.5/JHEMCU F722HD AIO/Hovering/24k.bbl")}
    assert "où le PID travaille" in f["Raie à 67 Hz encore visible après filtrage"].detail
    # Kris : mécanique (raie fixe) avant les passe-bas ; LPF1 du gyro coupable même en stationnaire
    first = report("1. PTB/Kris/13. Hover.bbl")[0]
    stages = [a.split(" — ")[0].split(". ", 1)[1] for a in first.actions[:-1]]
    assert stages[0] == "Mécanique" and "Passe-bas du gyro" in stages
    assert stages.index("Filtre RPM") < stages.index("Passe-bas du gyro") if "Filtre RPM" in stages else True


def test_log_rate_rule():
    from opentunebox.advisor.rules import ATTENTION, analyze, log_rate_needed

    # 8 kHz de gyro (PID 4 ou 8 kHz) → 2 kHz ; 3,2 kHz → 1,6 kHz ; whoop à 480 Hz → 3e harmonique visible
    assert log_rate_needed(4000) == 2000 and log_rate_needed(8000) == 2000 and log_rate_needed(3200) == 1600
    assert log_rate_needed(4000, 480) == 4000 and log_rate_needed(3200, 210) == 1600
    # Kris : 993 Hz pour une boucle PID à 4 kHz → attention, 1/2 ; aucun allègement tiré d'un spectre replié
    lg = _load("1. PTB/Kris/BTFL_BLACKBOX_LOG_TP_20260914_181415_F722_AIO.bbl")
    f = {x.title: x for x in analyze(lg).findings}
    assert f["Fréquence d'enregistrement trop basse"].severity == ATTENTION
    assert f["Fréquence d'enregistrement trop basse"].actions[0].startswith("blackbox_sample_rate = 1/2")
    assert not {"Filtre RPM plus fort que nécessaire", "Le LPF1 du gyro peut être coupé",
                "Notch dynamique : moins de notchs suffiraient"} & set(f)
    assert f["Stratégie de filtrage"].actions[0].startswith("1. Enregistrement")


def test_imbalance_interpretation():
    from opentunebox.advisor.rules import _imbalance_finding

    front = _imbalance_finding([20, 30, 20, 30], 20)
    assert "avant" in front.detail
    diag = _imbalance_finding([30, 20, 20, 30], 20)
    assert "diagonale" in diag.detail


def test_tool_windows_render():
    from PySide6.QtWidgets import QApplication

    from opentunebox.gui.state import AppState
    from opentunebox.gui.tools.advisor_panel import AdvisorWindow
    from opentunebox.gui.tools.flight_stats import FlightStatsWindow
    from opentunebox.gui.tools.setup_info import SetupInfoWindow

    app = QApplication.instance() or QApplication([])
    st = AppState()
    st.add(_load("1. PTB/BlackSummer/Damping 0.8.BFL"))
    st.add(_load("1. PTB/BlackSummer/Damping 1.4.BFL"))
    wins = []
    for cls in (FlightStatsWindow, AdvisorWindow):
        w = cls(st)
        w.files.selectAll() if w.multi_file else None
        w.render(w.compute(w.files.selected_entries()))
        wins.append(w)
    s = SetupInfoWindow(st)
    s.files.selectAll()
    s.only_diff.setChecked(True)
    assert s.tree.topLevelItemCount() >= 2  # PID et curseurs diffèrent
    wins.append(s)
    for w in wins:
        w.close()
        w.deleteLater()
    app.processEvents()


def test_betaflight_chirp_measured_in_angle_mode():
    # Chirp de Betaflight en mode angle : l'excitation (debug CHIRP) s'ajoute à la commande radio comme
    # instrument ; sans elle, la cohérence est insuffisante et la mesure retombe sur la méthode approchée
    from opentunebox.analysis.loop import MEASURED
    from opentunebox.analysis.period import auto_period
    from opentunebox.analysis.step import step_response

    lg = _load("Cedric/BF Chirp/Air65.bbl")
    sel = lg.slice_time(*auto_period(lg))
    terms = lg.pid_terms(0, sel)
    assert lg.chirp_excitation(0, sel) is not None and terms.instrument is not None
    lp = step_response(lg.setpoint[0, sel], lg.gyro[0, sel], lg.fs, terms=terms).loop
    assert lp.method == MEASURED and not lp.sensitivity_peak_at_edge
    assert 20 < lp.crossover_hz < 27 and 34 < lp.phase_margin_deg < 46  # 23,6 Hz, 39,5° (2026-10-06)
    assert _load("Cedric/JeNo 5/FINAL.bbl").chirp_excitation(0) is None


def test_noise_report_classifies_every_title():
    """Chaque constat de bruit ou de filtre de l'assistant a sa place dans le diagnostic de la fenêtre Spectre."""
    import re
    from pathlib import Path

    from opentunebox.advisor import noise, rules
    from opentunebox.advisor.rules import CONSEIL, Finding

    src = Path(rules.__file__).read_text(encoding="utf-8")
    titles = re.findall(r'Finding\("(Bruit|Filtres|Mécanique|Réponse)",\s*[^,]+,\s*f?"([^"{]*)', src)
    explicit = noise.CONDITION_TITLES + noise.DTERM_TITLES + noise.GYRO_TITLES + (noise.STRATEGY_TITLE,)
    for cat, title in titles:
        f = Finding(cat, CONSEIL, title, "")
        if cat in ("Réponse", "Mécanique") and not title.startswith(noise.GYRO_TITLES):
            assert noise.section_of(f) is None, title
        else:
            assert title.startswith(explicit), f"constat non rangé : {title}"
    assert noise.section_of(Finding("Filtres", CONSEIL, "Retard des filtres D", "")) == "dterm"
    assert noise.section_of(Finding("Filtres", CONSEIL, "Retard des filtres gyro", "")) == "gyro"
    assert noise.section_of(Finding("Enregistrement", CONSEIL, "Gyro non filtré absent", "")) == "conditions"


def test_noise_report_strategy_fallback():
    from opentunebox.advisor.noise import noise_report

    nr = noise_report(_load("Cedric/JeNo 5/FINAL.bbl"))
    assert nr.moving and nr.has_unfiltered and nr.harmonics is not None
    assert [f.title for f in nr.dterm_findings][0] == "Terme D bruité"
    # une seule étape de filtrage : pas de « Stratégie de filtrage » de l'assistant, mais un conseil quand même
    assert len(nr.strategy) == 1 and nr.strategy[0].title == "Un seul réglage de filtre à faire"
    assert "filtre D" in nr.strategy[0].actions[0]
