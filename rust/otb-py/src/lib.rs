//! Pont PyO3 : module `opentunebox._native` (D-031). Aucune logique ici : les calculs sont dans les autres
//! crates, ce fichier convertit seulement les tableaux numpy.

use pyo3::prelude::*;

#[pymodule]
mod _native {
    use numpy::ndarray::Array2;
    use numpy::{IntoPyArray, PyArray1, PyArray2, PyReadonlyArray1};
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
}
