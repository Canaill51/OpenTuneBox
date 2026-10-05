#!/usr/bin/env python3
"""Recalcule les statistiques du corpus utilisées pour calibrer les seuils de l'assistant.

Usage : calibrate_advisor.py NB_FICHIERS SORTIE.json  (dossier de logs : variable OTB_LOGS)
        calibrate_advisor.py --resume SORTIE.json      (p10 / p50 / p90 par taille, voir docs/assistant.md)
"""
import sys, json, glob, os, random
import numpy as np
from opentunebox.log.flightlog import load
from opentunebox.analysis.period import auto_period
from opentunebox.analysis.spectral import spectrum
from opentunebox.analysis.step import step_response
from opentunebox.analysis.delay import compute_delays
from opentunebox.analysis.motors import motor_noise
from opentunebox.advisor.profiles import PROFILES, detect_profile, profile_key_from_hz
from opentunebox.advisor.rules import frame_resonance
from opentunebox.analysis.motors import hover_motor_hz
from opentunebox.analysis.stats import flight_stats

def summary(path):
    """Affiche, par taille puis pour tout le corpus (TOUS) : p10 / p50 / p90 par session (roll et pitch
    regroupés) et p10 / p90 avec une voix par drone (médiane de ses sessions ; drone = nom, sinon dossier)."""
    rows=json.load(open(path))
    for r in rows:
        if r.get("hover_hz"): r["profile"]=profile_key_from_hz(r["hover_hz"])  # limites actuelles
        r["drone"]=(r.get("craft") or "").strip() or "?"+os.path.dirname(r["file"])
        # mesures dérivées, telles que les règles les utilisent
        if "peak_0" in r and "peak_1" in r: r["peakdiff"]=abs(r["peak_1"]-r["peak_0"])
        if "imean_0" in r: r["imean_abs"]=max(abs(r["imean_0"]),abs(r["imean_1"]))
        for a in range(2):
            if f"mnoise_{a}" in r: r[f"mleft_{a}"]=r[f"mnoise_{a}"][0]
        if r.get("res_db") is not None and not r.get("res_near"): r["res_free_db"]=r["res_db"]
    keys=sorted({k.rsplit("_",1)[0] if k[-2:] in ("_0","_1") else k for r in rows for k in r}
                - {"file","sess","profile","craft","drone","res_near","mnoise","imean"})
    def vals(sub,k):
        return [x for r in sub for n in (k,k+"_0",k+"_1") for x in [r.get(n)]
                if isinstance(x,(int,float)) and not isinstance(x,bool) and np.isfinite(x)]
    for key in [*PROFILES,"TOUS"]:
        sub=[r for r in rows if key=="TOUS" or r["profile"]==key]
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

if sys.argv[1]=="--resume":
    summary(sys.argv[2]); sys.exit()
root=os.environ.get("OTB_LOGS", os.path.join(os.path.expanduser("~"), "Documents", "Drones", "Logs"))
files=[f for f in glob.glob(root+"/**/*", recursive=True) if f.lower().endswith((".bbl",".bfl"))]
random.seed(0); random.shuffle(files)
out=[]
for f in files[:int(sys.argv[1])]:
    try:
        for lg in load(f):
            if lg.duration < 8: continue
            t0,t1=auto_period(lg); sel=lg.slice_time(t0,t1)
            if sel.stop-sel.start < lg.fs*5: continue
            prof,_=detect_profile(lg)
            r={"file":os.path.relpath(f,root),"sess":lg.session,"profile":prof.key,"fs":lg.fs,
               "craft":lg.settings.craft_name,"hover_hz":hover_motor_hz(lg),
               "corrupt":lg.stats.get("corrupt_frames",0)}
            nyq=lg.fs/2
            def band(x,lo,hi):
                s=spectrum(x,lg.fs,resolution_hz=2); m=(s.freqs>=lo)&(s.freqs<=min(hi,nyq*0.95)); return float(np.mean(s.values[m]))
            for a in range(2):
                r[f"gyro_{a}"]=band(lg.gyro[a,sel],80,500)
                if lg.gyro_unfilt is not None: r[f"gyrou_{a}"]=band(lg.gyro_unfilt[a,sel],80,500)
                if np.any(lg.d_term[a]): r[f"dterm_{a}"]=band(lg.d_term[a,sel],80,500)
                r[f"lowband_{a}"]=band(lg.gyro[a,sel],20,80)
                st=step_response(lg.setpoint[a,sel],lg.gyro[a,sel],lg.fs)
                if st.n>=20:
                    r[f"peak_{a}"]=st.metrics.peak; r[f"lat_{a}"]=st.metrics.latency_ms
                    r[f"smax_{a}"]=st.loop.sensitivity_peak_db; r[f"pm_{a}"]=st.loop.phase_margin_deg
                if lg.motor_hz is not None:
                    mn=motor_noise(lg.gyro[a,sel],lg.motor_hz[:,sel],lg.fs); r[f"mnoise_{a}"]=mn.mean_db.tolist()
            d=compute_delays(lg,sel); r["gdelay"]=d.gyro_ms; r["ddelay"]=d.dterm_ms
            fst=flight_stats(lg,sel); r["sat"]=fst.motor_saturation_pct; r["imb"]=fst.motor_imbalance_pct
            for a in range(2): r[f"imean_{a}"]=float(np.mean(lg.i_term[a,sel]))
            res=frame_resonance(lg,sel)
            if res is not None: r["res_hz"],r["res_db"],r["res_near"]=res
            out.append(r)
    except Exception as e:
        print("ERR",f,repr(e)[:150], file=sys.stderr)
json.dump(out,open(sys.argv[2],"w"))
print(len(out),"sessions")
