//! Pont PyO3 : module `opentunebox._native` (D-031). Aucune logique ici : les calculs sont dans les autres
//! crates, ce fichier convertit seulement les tableaux numpy.

use pyo3::prelude::*;

#[pymodule]
mod _native {
    use numpy::ndarray::Array2;
    use numpy::{
        Complex64, IntoPyArray, PyArray1, PyArray2, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods,
    };
    use otb_decoder::{CTX_SIZE, FrameDef, Params, Table};
    use pyo3::exceptions::{PyRuntimeError, PyValueError};
    use pyo3::prelude::*;

    type Out<'py> = (
        Bound<'py, PyArray2<i64>>,
        Bound<'py, PyArray1<i8>>,
        Bound<'py, PyArray2<i64>>,
        Bound<'py, PyArray1<i64>>,
        Bound<'py, PyArray2<i64>>,
        Bound<'py, PyArray1<i64>>,
        Bound<'py, PyArray2<i64>>,
        Bound<'py, PyArray1<i64>>,
    );

    fn array<'py>(py: Python<'py>, t: Table) -> PyResult<Bound<'py, PyArray2<i64>>> {
        let shape = (t.rows(), t.cols);
        let a = Array2::from_shape_vec(shape, t.data).map_err(|e| PyValueError::new_err(e.to_string()))?;
        Ok(a.into_pyarray(py))
    }

    fn slice<'a, T: numpy::Element>(a: &'a PyReadonlyArray1<'_, T>, name: &str) -> PyResult<&'a [T]> {
        a.as_slice()
            .map_err(|_| PyValueError::new_err(format!("{name} : tableau non contigu")))
    }

    /// Exécute un calcul sans le GIL ; une erreur de taille devient `ValueError`, une panique (bug)
    /// `RuntimeError` ordinaire plutôt qu'une `PanicException` qui échappe aux `except Exception` (L-032).
    fn guarded<T: Send>(
        py: Python<'_>,
        f: impl FnOnce() -> Result<T, String> + Send + std::panic::UnwindSafe,
    ) -> PyResult<T> {
        py.detach(|| std::panic::catch_unwind(f))
            .map_err(|_| PyRuntimeError::new_err("calcul : erreur interne"))?
            .map_err(PyValueError::new_err)
    }

    fn def<'a>(
        pred: &'a PyReadonlyArray1<'_, i64>,
        enc: &'a PyReadonlyArray1<'_, i64>,
        n: usize,
        name: &str,
    ) -> PyResult<FrameDef<'a>> {
        let (pred, enc) = (slice(pred, name)?, slice(enc, name)?);
        if pred.len() < n || enc.len() < n {
            return Err(PyValueError::new_err(format!(
                "{name} : définition plus courte que {n} champs"
            )));
        }
        Ok(FrameDef { pred, enc, n })
    }

    /// Même signature et mêmes résultats que `opentunebox.decoder.core.decode_stream`.
    #[pyfunction]
    #[allow(clippy::too_many_arguments)]
    fn decode_stream<'py>(
        py: Python<'py>,
        buf: PyReadonlyArray1<'py, u8>,
        start: i64,
        end: i64,
        i_pred: PyReadonlyArray1<'py, i64>,
        i_enc: PyReadonlyArray1<'py, i64>,
        p_pred: PyReadonlyArray1<'py, i64>,
        p_enc: PyReadonlyArray1<'py, i64>,
        n_main: usize,
        s_pred: PyReadonlyArray1<'py, i64>,
        s_enc: PyReadonlyArray1<'py, i64>,
        n_slow: usize,
        g_pred: PyReadonlyArray1<'py, i64>,
        g_enc: PyReadonlyArray1<'py, i64>,
        n_gps: usize,
        h_pred: PyReadonlyArray1<'py, i64>,
        h_enc: PyReadonlyArray1<'py, i64>,
        n_home: usize,
        has_p: bool,
        has_s: bool,
        has_g: bool,
        has_h: bool,
        i_interval: i64,
        p_num: i64,
        p_denom: i64,
        ctx: PyReadonlyArray1<'py, i64>,
        est_rows: usize,
    ) -> PyResult<Out<'py>> {
        let ctx_s = slice(&ctx, "ctx")?;
        if ctx_s.len() < CTX_SIZE {
            return Err(PyValueError::new_err("ctx : tableau trop court"));
        }
        let mut ctx_a = [0i64; CTX_SIZE];
        ctx_a.copy_from_slice(&ctx_s[..CTX_SIZE]);
        let prm = Params {
            start,
            end,
            i_def: def(&i_pred, &i_enc, n_main, "I")?,
            p_def: def(&p_pred, &p_enc, if has_p { n_main } else { 0 }, "P")?,
            s_def: def(&s_pred, &s_enc, n_slow, "S")?,
            g_def: def(&g_pred, &g_enc, n_gps, "G")?,
            h_def: def(&h_pred, &h_enc, n_home, "H")?,
            has_p,
            has_s,
            has_g,
            has_h,
            i_interval,
            p_num,
            p_denom,
            ctx: ctx_a,
            est_rows,
        };
        let buf = slice(&buf, "buf")?;
        // une panique du décodeur (bug) devient une exception ordinaire, rattrapée par l'application
        let d = py
            .detach(|| std::panic::catch_unwind(|| otb_decoder::decode_stream(buf, &prm)))
            .map_err(|_| PyRuntimeError::new_err("décodeur : erreur interne (trames illisibles ?)"))?;
        let events: Vec<i64> = d.events.iter().flatten().copied().collect();
        let n_events = d.events.len();
        Ok((
            array(py, d.main)?,
            d.kind.into_pyarray(py),
            array(py, d.slow)?,
            d.slow_at.into_pyarray(py),
            array(py, d.gps)?,
            d.gps_at.into_pyarray(py),
            Array2::from_shape_vec((n_events, 4), events)
                .map_err(|e| PyValueError::new_err(e.to_string()))?
                .into_pyarray(py),
            d.stats.to_vec().into_pyarray(py),
        ))
    }

    /// Même résultat que `bf_filters._biquad` (numba) : coefficients (b0, b1, b2, a1, a2) d'un étage.
    #[pyfunction]
    fn biquad(kind: i64, f: f64, q: f64, w: f64, fs: f64) -> (f64, f64, f64, f64, f64) {
        let [b0, b1, b2, a1, a2] = otb_dsp::biquad(kind, f, q, w, fs);
        (b0, b1, b2, a1, a2)
    }

    /// Même résultat que `bf_filters._sections_response` (numba).
    #[pyfunction]
    fn sections_response<'py>(
        py: Python<'py>,
        kinds: PyReadonlyArray1<'py, i64>,
        fcs: PyReadonlyArray1<'py, f64>,
        qs: PyReadonlyArray1<'py, f64>,
        ws: PyReadonlyArray1<'py, f64>,
        rates: PyReadonlyArray1<'py, f64>,
        freqs: PyReadonlyArray1<'py, f64>,
    ) -> PyResult<Bound<'py, PyArray1<Complex64>>> {
        let st = otb_dsp::Sections {
            kinds: slice(&kinds, "kinds")?,
            fcs: slice(&fcs, "fcs")?,
            qs: slice(&qs, "qs")?,
            ws: slice(&ws, "ws")?,
            rates: slice(&rates, "rates")?,
        };
        let freqs = slice(&freqs, "freqs")?;
        let (re, im) = guarded(py, || otb_dsp::sections_response(&st, freqs))?;
        let h: Vec<Complex64> = re.iter().zip(&im).map(|(&r, &i)| Complex64::new(r, i)).collect();
        Ok(h.into_pyarray(py))
    }

    /// Même résultat que `bf_filters._run_section` (numba) : un étage biquad appliqué à x (axes, n).
    #[pyfunction]
    #[allow(clippy::too_many_arguments)]
    fn run_section<'py>(
        py: Python<'py>,
        x: PyReadonlyArray2<'py, f64>,
        kind: i64,
        freq: PyReadonlyArray1<'py, f64>,
        q: f64,
        weight: PyReadonlyArray1<'py, f64>,
        fs: f64,
        f_limit: f64,
    ) -> PyResult<Bound<'py, PyArray2<f64>>> {
        let (n_ax, n_cols) = (x.shape()[0], x.shape()[1]);
        // ⚠️ `as_slice` accepte aussi l'ordre « colonne par colonne » (Fortran) : lu ligne par ligne, il donnerait
        // un résultat faux sans erreur (L-036)
        if !x.is_c_contiguous() {
            return Err(PyValueError::new_err(
                "x : tableau non rangé ligne par ligne (C contigu)",
            ));
        }
        let xs = x
            .as_slice()
            .map_err(|_| PyValueError::new_err("x : tableau non contigu"))?;
        let (freq, weight) = (slice(&freq, "freq")?, slice(&weight, "weight")?);
        let y = guarded(py, || {
            otb_dsp::run_section(xs, n_ax, kind, freq, q, weight, fs, f_limit)
        })?;
        let n = n_cols;
        Array2::from_shape_vec((n_ax, n), y)
            .map_err(|e| PyValueError::new_err(e.to_string()))
            .map(|a| a.into_pyarray(py))
    }

    /// Même résultat que `bf_filters._pt1_track` (numba) : PT1 sur un signal 1D.
    #[pyfunction]
    fn pt1_track<'py>(py: Python<'py>, x: PyReadonlyArray1<'py, f64>, k: f64) -> PyResult<Bound<'py, PyArray1<f64>>> {
        let xs = slice(&x, "x")?;
        let y = guarded(py, || Ok(otb_dsp::pt1_track(xs, k)))?;
        Ok(y.into_pyarray(py))
    }

    /// Même résultat que `tests/ref_step.py:step_windows_numpy` : réponses indicielles des fenêtres `starts`
    /// (D-031, phase 2b). → (R (k, lr), maxin (k,), sum_gh complexe, sum_hh, sum_gg).
    #[pyfunction]
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    fn step_windows<'py>(
        py: Python<'py>,
        sp: PyReadonlyArray1<'py, f64>,
        gy: PyReadonlyArray1<'py, f64>,
        w: PyReadonlyArray1<'py, f64>,
        starts: PyReadonlyArray1<'py, i64>,
        l: usize,
        lr: usize,
        pad: usize,
        reg: f64,
        y_correction: bool,
        steady_lo: usize,
        steady_hi: usize,
    ) -> PyResult<(
        Bound<'py, PyArray2<f64>>,
        Bound<'py, PyArray1<f64>>,
        Bound<'py, PyArray1<Complex64>>,
        Bound<'py, PyArray1<f64>>,
        Bound<'py, PyArray1<f64>>,
    )> {
        let (sp, gy, w, starts) = (
            slice(&sp, "sp")?,
            slice(&gy, "gy")?,
            slice(&w, "w")?,
            slice(&starts, "starts")?,
        );
        let p = otb_dsp::step::StepParams {
            w,
            l,
            lr,
            pad,
            reg,
            y_correction,
            steady: (steady_lo, steady_hi),
        };
        let out = guarded(py, || otb_dsp::step::step_windows(sp, gy, starts, &p))?;
        let k = out.maxin.len();
        let rows = Array2::from_shape_vec((k, lr), out.rows).map_err(|e| PyValueError::new_err(e.to_string()))?;
        let gh: Vec<Complex64> = out
            .sum_gh_re
            .iter()
            .zip(&out.sum_gh_im)
            .map(|(&r, &i)| Complex64::new(r, i))
            .collect();
        Ok((
            rows.into_pyarray(py),
            out.maxin.into_pyarray(py),
            gh.into_pyarray(py),
            out.sum_hh.into_pyarray(py),
            out.sum_gg.into_pyarray(py),
        ))
    }
}
