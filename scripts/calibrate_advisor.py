#!/usr/bin/env python3
"""Recalcule les statistiques du corpus utilisées pour calibrer les seuils de l'assistant.

Usage : calibrate_advisor.py NB_FICHIERS SORTIE.json  (dossier de logs : variable OTB_LOGS)
         (puis lire les percentiles, voir docs/assistant.md)
"""
import sys, json, glob, os, random
import numpy as np
from opentunebox.log.flightlog import load
from opentunebox.analysis.period import auto_period
from opentunebox.analysis.spectral import spectrum
from opentunebox.analysis.step import step_response
from opentunebox.analysis.delay import compute_delays
from opentunebox.analysis.motors import motor_noise
from opentunebox.advisor.profiles import detect_profile
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
            r={"file":os.path.relpath(f,root),"sess":lg.session,"profile":prof.key,"fs":lg.fs}
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
            r["sat"]=float(np.mean(np.any(lg.motors[:,sel]>=98,axis=0))*100) if lg.n_motors else None
            out.append(r)
    except Exception as e:
        print("ERR",f,repr(e)[:150], file=sys.stderr)
json.dump(out,open(sys.argv[2],"w"))
print(len(out),"sessions")
