/*
 * Banc de référence des briques de filtrage de Betaflight (lot 24 du plan, D-062).
 *
 * Ce fichier est écrit par le projet OpenTuneBox (licence PolyForm Noncommercial 1.0.0) : il ne contient aucune ligne du
 * firmware. `otb-dev firmware-bench` le compile, hors du dépôt, avec `common/filter.c` et `common/maths.c` téléchargés à la
 * version voulue, puis ne garde que les nombres qu'il écrit. Le programme compilé n'est jamais livré ni versionné.
 *
 * Appel : betaflight_filters TYPE SORTIE [@ENTREE ...] [nom=valeur ...]
 *   - TYPE : le cas joué (voir `main`) ; SORTIE : fichier de float32 petit-boutistes écrit ;
 *   - @ENTREE : fichiers de float32, dans l'ordre attendu par le cas ;
 *   - nom=valeur : paramètres (lus par `strtof`, arrondi au plus près : la valeur float32 écrite par Rust revient exacte).
 *
 * Le passage cadence gyro → cadence PID (`two_rate`) recopie `sensors/gyro.c` et `gyro_filter_impl.c`, que le banc ne compile
 * pas : il vérifie les briques du firmware enchaînées par cette recopie, la même que `Decimator` côté Rust.
 * Famille : OTB_SVF défini (2026.6 et après) → filtres SVF ; sinon biquads.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "common/filter.h"
#include "common/maths.h"

#define MAX_INPUTS 8
#define MAX_PARAMS 16

static float *inputs[MAX_INPUTS];
static size_t input_len[MAX_INPUTS];
static int n_inputs;
static const char *param_name[MAX_PARAMS];
static const char *param_value[MAX_PARAMS];
static int n_params;
static float *out;
static size_t out_len;
static size_t out_cap;

static void fail(const char *msg, const char *what)
{
    fprintf(stderr, "betaflight_filters : %s %s\n", msg, what ? what : "");
    exit(2);
}

static float *read_f32(const char *path, size_t *len)
{
    FILE *f = fopen(path, "rb");
    if (!f) fail("entrée illisible", path);
    fseek(f, 0, SEEK_END);
    long bytes = ftell(f);
    fseek(f, 0, SEEK_SET);
    float *buf = malloc((size_t)bytes + sizeof(float));
    if (!buf || fread(buf, 1, (size_t)bytes, f) != (size_t)bytes) fail("lecture impossible", path);
    fclose(f);
    *len = (size_t)bytes / sizeof(float);
    return buf;
}

static const char *param(const char *name)
{
    for (int i = 0; i < n_params; i++) {
        if (strcmp(param_name[i], name) == 0) return param_value[i];
    }
    fail("paramètre manquant", name);
    return NULL;
}

static float pf(const char *name) { return strtof(param(name), NULL); }
static uint32_t pu(const char *name) { return (uint32_t)strtoul(param(name), NULL, 10); }
static int pi_(const char *name) { return (int)strtol(param(name), NULL, 10); }

static float *in(int i, size_t *len)
{
    if (i >= n_inputs) fail("entrée manquante", NULL);
    *len = input_len[i];
    return inputs[i];
}

static void emit(float v)
{
    if (out_len == out_cap) {
        out_cap = out_cap ? 2 * out_cap : 4096;
        out = realloc(out, out_cap * sizeof(float));
        if (!out) fail("mémoire", NULL);
    }
    out[out_len++] = v;
}

/* Passe-bas PTn ou biquad / SVF selon `type` (1 PT1, 2 biquad ou SVF, 3 PT2, 4 PT3), à `looptime` µs. */
typedef struct {
    int type;
    pt1Filter_t pt1;
    pt2Filter_t pt2;
    pt3Filter_t pt3;
#ifdef OTB_SVF
    svfLowpassFilter_t svf;
#else
    biquadFilter_t bq;
#endif
} lowpass_t;

static void lowpass_init(lowpass_t *l, int type, float hz, uint32_t looptime)
{
    const float dt = looptime * 1e-6f;
    l->type = type;
    switch (type) {
    case 1: pt1FilterInit(&l->pt1, pt1FilterGain(hz, dt)); break;
#ifdef OTB_SVF
    case 2: svfLowpassFilterInit(&l->svf, hz, dt); break;
#else
    case 2: biquadFilterInitLPF(&l->bq, hz, looptime); break;
#endif
    case 3: pt2FilterInit(&l->pt2, pt2FilterGain(hz, dt)); break;
    case 4: pt3FilterInit(&l->pt3, pt3FilterGain(hz, dt)); break;
    default: break;
    }
}

static float lowpass_apply(lowpass_t *l, float x)
{
    switch (l->type) {
    case 1: return pt1FilterApply(&l->pt1, x);
#ifdef OTB_SVF
    case 2: return svfLowpassFilterApply(&l->svf, x);
#else
    case 2: return biquadFilterApply(&l->bq, x);
#endif
    case 3: return pt2FilterApply(&l->pt2, x);
    case 4: return pt3FilterApply(&l->pt3, x);
    default: return x;
    }
}

static void run_pt(int dynamic)
{
    size_t n, nt = 0;
    const float *x = in(0, &n);
    const float *track = dynamic ? in(1, &nt) : NULL;
    const int order = pi_("order");
    const float dt = pu("looptime_us") * 1e-6f;
    const float hz = dynamic ? track[0] : pf("hz");
    pt1Filter_t p1;
    pt2Filter_t p2;
    pt3Filter_t p3;
    pt1FilterInit(&p1, pt1FilterGain(hz, dt));
    pt2FilterInit(&p2, pt2FilterGain(hz, dt));
    pt3FilterInit(&p3, pt3FilterGain(hz, dt));
    for (size_t i = 0; i < n; i++) {
        if (dynamic) {
            pt1FilterUpdateCutoff(&p1, pt1FilterGain(track[i], dt));
            pt2FilterUpdateCutoff(&p2, pt2FilterGain(track[i], dt));
            pt3FilterUpdateCutoff(&p3, pt3FilterGain(track[i], dt));
        }
        emit(order == 1 ? pt1FilterApply(&p1, x[i]) : order == 2 ? pt2FilterApply(&p2, x[i]) : pt3FilterApply(&p3, x[i]));
    }
}

#ifdef OTB_SVF

static void run_svf_lpf(int dynamic)
{
    size_t n, nt = 0;
    const float *x = in(0, &n);
    const float *track = dynamic ? in(1, &nt) : NULL;
    const float dt = pu("looptime_us") * 1e-6f;
    svfLowpassFilter_t f;
    svfLowpassFilterInit(&f, dynamic ? track[0] : pf("hz"), dt);
    for (size_t i = 0; i < n; i++) {
        if (dynamic) svfLowpassFilterUpdate(&f, track[i], dt);
        emit(svfLowpassFilterApply(&f, x[i]));
    }
}

static void run_svf_notch(int dynamic)
{
    size_t n, nt = 0;
    const float *x = in(0, &n);
    const float *track = dynamic ? in(1, &nt) : NULL;
    const float dt = pu("looptime_us") * 1e-6f;
    svfNotchFilter_t f;
    if (dynamic) {
        const float q = pf("q");
        svfNotchInit(&f, track[0], dt, q);
        for (size_t i = 0; i < n; i++) {
            svfNotchUpdate(&f, track[i], dt, q);
            emit(svfNotchApply(&f, x[i]));
        }
    } else {
        const float center = pf("center_hz");
        svfNotchInit(&f, center, dt, filterGetNotchQ(center, pf("cutoff_hz")));
        for (size_t i = 0; i < n; i++) emit(svfNotchApply(&f, x[i]));
    }
}

static void run_rpm_notch(void)
{
    size_t n, n1, n2, nt, nw;
    const float *x0 = in(0, &n);
    const float *x1 = in(1, &n1);
    const float *x2 = in(2, &n2);
    const float *track = in(3, &nt);
    const float *weight = in(4, &nw);
    const float dt = pu("looptime_us") * 1e-6f;
    const float q = pf("q");
    rpmNotch_t f;
    rpmNotchInit(&f, pf("init_hz"), dt, q, 0.0f);
    float *y = malloc(3 * n * sizeof(float));
    for (size_t i = 0; i < n; i++) {
        rpmNotchUpdate(&f, track[i], dt, q, weight[i]);
        float v[3] = { x0[i], x1[i], x2[i] };
        rpmNotchApply(&f, v);
        y[i] = v[0];
        y[n + i] = v[1];
        y[2 * n + i] = v[2];
    }
    for (size_t i = 0; i < 3 * n; i++) emit(y[i]);
    free(y);
}

#else

static void run_biquad_lpf(int dynamic)
{
    size_t n, nt = 0;
    const float *x = in(0, &n);
    const float *track = dynamic ? in(1, &nt) : NULL;
    const uint32_t looptime = pu("looptime_us");
    biquadFilter_t f;
    biquadFilterInitLPF(&f, dynamic ? track[0] : pf("hz"), looptime);
    const int df1 = dynamic || pi_("df1");
    for (size_t i = 0; i < n; i++) {
        if (dynamic) biquadFilterUpdateLPF(&f, track[i], looptime);
        emit(df1 ? biquadFilterApplyDF1(&f, x[i]) : biquadFilterApply(&f, x[i]));
    }
}

static void run_biquad_notch(int dynamic)
{
    size_t n, nt = 0;
    const float *x = in(0, &n);
    const float *track = dynamic ? in(1, &nt) : NULL;
    const uint32_t looptime = pu("looptime_us");
    biquadFilter_t f;
    if (dynamic) {
        const float q = pf("q");
        biquadFilterInit(&f, track[0], looptime, q, FILTER_NOTCH, 1.0f);
        for (size_t i = 0; i < n; i++) {
            biquadFilterUpdate(&f, track[i], looptime, q, FILTER_NOTCH, 1.0f);
            emit(biquadFilterApplyDF1(&f, x[i]));
        }
    } else {
        const float center = pf("center_hz");
        biquadFilterInit(&f, center, looptime, filterGetNotchQ(center, pf("cutoff_hz")), FILTER_NOTCH, 1.0f);
        for (size_t i = 0; i < n; i++) emit(biquadFilterApply(&f, x[i]));
    }
}

static void run_biquad_rpm(void)
{
    size_t n, nt, nw;
    const float *x = in(0, &n);
    const float *track = in(1, &nt);
    const float *weight = in(2, &nw);
    const uint32_t looptime = pu("looptime_us");
    const float q = pf("q");
    biquadFilter_t f;
    biquadFilterInit(&f, pf("init_hz"), looptime, q, FILTER_NOTCH, 0.0f);
    for (size_t i = 0; i < n; i++) {
        biquadFilterUpdate(&f, track[i], looptime, q, FILTER_NOTCH, weight[i]);
        emit(biquadFilterApplyDF1Weighted(&f, x[i]));
    }
}

#endif

/* Notch statique (centre, coupure) à `looptime` µs, ou rien quand center_hz = 0. */
typedef struct {
    int on;
#ifdef OTB_SVF
    svfNotchFilter_t f;
#else
    biquadFilter_t f;
#endif
} notch_t;

static void notch_init(notch_t *nt, float center, float cutoff, uint32_t looptime)
{
    nt->on = center > 0.0f;
    if (!nt->on) return;
#ifdef OTB_SVF
    svfNotchInit(&nt->f, center, looptime * 1e-6f, filterGetNotchQ(center, cutoff));
#else
    biquadFilterInit(&nt->f, center, looptime, filterGetNotchQ(center, cutoff), FILTER_NOTCH, 1.0f);
#endif
}

static float notch_apply(notch_t *nt, float x)
{
    if (!nt->on) return x;
#ifdef OTB_SVF
    return svfNotchApply(&nt->f, x);
#else
    return biquadFilterApply(&nt->f, x);
#endif
}

/* Deux cadences : LPF2 à la cadence gyro, décimation, notch statique et LPF1 à la cadence PID. */
static void run_two_rate(void)
{
    size_t n;
    const float *x = in(0, &n);
    const uint32_t gyro_us = pu("gyro_us");
    const uint32_t denom = pu("denom");
    const uint32_t pid_us = gyro_us * denom;
    lowpass_t lpf2, lpf1;
    notch_t notch;
    lowpass_init(&lpf2, pi_("lpf2_type"), pf("lpf2_hz"), gyro_us);
    notch_init(&notch, pf("notch_center_hz"), pf("notch_cutoff_hz"), pid_us);
    lowpass_init(&lpf1, pi_("lpf1_type"), pf("lpf1_hz"), pid_us);
    const int filtered = lpf2.type != 0;
    float sum = 0.0f;
    uint8_t count = 0;
    for (size_t i = 0; i < n; i++) {
        if (filtered) {
            sum = lowpass_apply(&lpf2, x[i]);
        } else {
            sum += x[i];
            count++;
        }
        if ((i + 1) % denom == 0) {
            float y = 0.0f;
            if (filtered) {
                y = sum;
            } else {
                if (count) y = sum / count;
                sum = 0.0f;
            }
            count = 0;
            y = notch_apply(&notch, y);
            emit(lowpass_apply(&lpf1, y));
        }
    }
}

int main(int argc, char **argv)
{
    if (argc < 3) fail("usage : TYPE SORTIE [@ENTREE ...] [nom=valeur ...]", NULL);
    const char *kind = argv[1];
    for (int i = 3; i < argc; i++) {
        if (argv[i][0] == '@') {
            if (n_inputs == MAX_INPUTS) fail("trop d'entrées", NULL);
            inputs[n_inputs] = read_f32(argv[i] + 1, &input_len[n_inputs]);
            n_inputs++;
        } else {
            char *eq = strchr(argv[i], '=');
            if (!eq || n_params == MAX_PARAMS) fail("paramètre mal écrit", argv[i]);
            *eq = '\0';
            param_name[n_params] = argv[i];
            param_value[n_params] = eq + 1;
            n_params++;
        }
    }

    size_t n, n2;
    if (strcmp(kind, "sincos") == 0) {
        const float *a = in(0, &n);
        for (size_t i = 0; i < n; i++) emit(sin_approx(a[i]));
        for (size_t i = 0; i < n; i++) emit(cos_approx(a[i]));
    } else if (strcmp(kind, "gains") == 0) {
        const float *fc = in(0, &n);
        const float *dt = in(1, &n2);
        for (size_t i = 0; i < n; i++) emit(pt1FilterGain(fc[i], dt[i]));
        for (size_t i = 0; i < n; i++) emit(pt2FilterGain(fc[i], dt[i]));
        for (size_t i = 0; i < n; i++) emit(pt3FilterGain(fc[i], dt[i]));
    } else if (strcmp(kind, "notch_q") == 0) {
        const float *c = in(0, &n);
        const float *f = in(1, &n2);
        for (size_t i = 0; i < n; i++) emit(filterGetNotchQ(c[i], f[i]));
    } else if (strcmp(kind, "pt") == 0) {
        run_pt(0);
    } else if (strcmp(kind, "pt_dyn") == 0) {
        run_pt(1);
    } else if (strcmp(kind, "two_rate") == 0) {
        run_two_rate();
#ifdef OTB_SVF
    } else if (strcmp(kind, "sincosf") == 0) {
        const float *a = in(0, &n);
        float *c = malloc(n * sizeof(float));
        for (size_t i = 0; i < n; i++) {
            float s;
            sincosf_approx(a[i], &s, &c[i]);
            emit(s);
        }
        for (size_t i = 0; i < n; i++) emit(c[i]);
        free(c);
    } else if (strcmp(kind, "svf_lpf") == 0) {
        run_svf_lpf(0);
    } else if (strcmp(kind, "svf_lpf_dyn") == 0) {
        run_svf_lpf(1);
    } else if (strcmp(kind, "svf_notch") == 0) {
        run_svf_notch(0);
    } else if (strcmp(kind, "svf_notch_dyn") == 0) {
        run_svf_notch(1);
    } else if (strcmp(kind, "rpm_notch") == 0) {
        run_rpm_notch();
#else
    } else if (strcmp(kind, "biquad_lpf") == 0) {
        run_biquad_lpf(0);
    } else if (strcmp(kind, "biquad_lpf_dyn") == 0) {
        run_biquad_lpf(1);
    } else if (strcmp(kind, "biquad_notch") == 0) {
        run_biquad_notch(0);
    } else if (strcmp(kind, "biquad_notch_dyn") == 0) {
        run_biquad_notch(1);
    } else if (strcmp(kind, "biquad_rpm") == 0) {
        run_biquad_rpm();
#endif
    } else {
        fail("type de cas inconnu pour cette famille :", kind);
    }

    FILE *f = fopen(argv[2], "wb");
    if (!f || fwrite(out, sizeof(float), out_len, f) != out_len) fail("écriture impossible", argv[2]);
    fclose(f);
    return 0;
}
