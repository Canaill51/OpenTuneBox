//! Banc de temps de cadre de l'interface (W-049) : la vraie `App`, les vraies pages, un vrai log, hors écran.
//!
//! `cargo run --release -p otb-gui --example banc_cadre -- LOG [--largeur 1920] [--cadres 60] [--gpu] [--glissement] [--zoom 8]`
//!
//! Mesure, cadre par cadre (`Harness::step`, un seul cadre ; `run` en enchaîne plusieurs) :
//! - l'écran « Guide », sans courbe : la part fixe de l'interface, à soustraire ;
//! - le viewer en vue entière sur la session **la plus longue** du log (la première peut être minuscule) ;
//! - avec `--glissement`, le viewer **pendant un glissement** : le pointeur déplace la vue en va-et-vient, donc les
//!   bornes du graphique changent à chaque cadre. C'est le cas qui saccade et celui qu'un cache de points décimés ne
//!   sait pas servir. Le banc **vérifie** que les bornes bougent vraiment (sinon il panique : une mesure de glissement
//!   immobile serait un faux « avant / après ») ;
//! - avec `--zoom F`, le viewer **zoomé** sur la fraction `1/F` du log : autant de points rendus et autant de courbes
//!   qu'en vue entière, mais `F` fois moins d'échantillons visibles. C'est la mesure qui dit si le cadre dépend du
//!   nombre d'échantillons (le même log sert de témoin à lui-même : comparer deux logs différents mélangerait le
//!   nombre de courbes présentes) ; le banc affiche les échantillons réellement visibles ;
//! - la tessellation (formes → triangles) du cadre du viewer, travail du processeur hors de `step` ;
//! - avec `--gpu`, le rendu `wgpu` (sur une vraie carte, voir `vraie_carte`) de l'écran Guide et du viewer, et leur
//!   différence (Vulkan absent de la CI, d'où l'option désactivée par défaut). Chaque rendu crée une texture et relit
//!   l'image (≈ 10 ms fixes sur une RTX 3050 Ti en 1920 × 1080, même sans courbe) : la différence est un **majorant
//!   bruité** de la part des courbes (de 1 à 4 ms d'un essai à l'autre sur `FINAL.bbl`), pas un temps de carte exact.
//!
//! Ce qui n'est **pas** mesuré : la synchronisation verticale et le compositeur (aucune fenêtre), la mémoire (affaire de
//! `otb-dev bench-open`). Le temps d'un cadre comprend la mise à jour de l'arbre d'accessibilité par `egui_kittest`, payée
//! aussi par l'écran Guide : la différence « viewer − Guide » l'élimine. Le seul chiffre comparable d'un log à l'autre est le
//! coût **par échantillon** de cette différence. Ne jamais lancer sous profileur pour décider (il gonfle les petits appels).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use otb_gui::app::{App, Screen};

/// Cadres joués avant de mesurer : polices, premier cadrage, caches de séries.
const CHAUFFE: usize = 3;
/// Une ouverture est finie quand aucune session n'est arrivée pendant ce délai.
const CALME: Duration = Duration::from_secs(2);
/// Délai au-delà duquel on abandonne (ouverture ou calcul du guide bloqués).
const LIMITE: Duration = Duration::from_secs(300);

struct Options {
    log: PathBuf,
    largeur: f32,
    cadres: usize,
    gpu: bool,
    glissement: bool,
    /// Fraction du log à afficher : 1 = vue entière (pas de mesure zoomée).
    zoom: f64,
}

fn options() -> Options {
    const USAGE: &str = "usage : banc_cadre LOG [--largeur 1920] [--cadres 60] [--gpu] [--glissement] [--zoom 8]";
    let mut args = std::env::args().skip(1);
    let (mut log, mut largeur, mut cadres, mut gpu) = (None, 1920.0, 60, false);
    let mut glissement = false;
    let mut zoom = 1.0f64;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--largeur" => largeur = args.next().and_then(|v| v.parse().ok()).expect(USAGE),
            "--cadres" => cadres = args.next().and_then(|v| v.parse().ok()).expect(USAGE),
            "--gpu" => gpu = true,
            "--glissement" => glissement = true,
            "--zoom" => zoom = args.next().and_then(|v| v.parse().ok()).expect(USAGE),
            _ if log.is_none() && !a.starts_with("--") => log = Some(PathBuf::from(a)),
            _ => panic!("{USAGE}"),
        }
    }
    assert!(cadres > 0, "{USAGE}");
    Options {
        log: log.expect(USAGE),
        largeur,
        cadres,
        gpu,
        glissement,
        zoom,
    }
}

/// Minimum et médiane, en millisecondes.
fn resume(mut ms: Vec<f64>) -> (f64, f64) {
    ms.sort_by(|a, b| a.total_cmp(b));
    (ms[0], ms[ms.len() / 2])
}

/// Joue `CHAUFFE` cadres, puis chronomètre `n` cadres un par un.
fn cadres(h: &mut Harness<'_, App>, n: usize) -> Vec<f64> {
    for _ in 0..CHAUFFE {
        h.step();
    }
    (0..n)
        .map(|_| {
            let t0 = Instant::now();
            h.step();
            t0.elapsed().as_secs_f64() * 1e3
        })
        .collect()
}

/// Joue `CHAUFFE` cadres, puis chronomètre `n` cadres **pendant un glissement** de la vue : le pointeur garde le
/// bouton enfoncé et va-et-vient horizontalement (± `AMPLITUDE_PX`), si bien que les bornes du graphique changent à
/// chaque cadre. Le va-et-vient garde la fenêtre proche de la vue entière : sans lui, un glissement d'un seul sens
/// finirait hors du log et mesurerait un graphique presque vide.
///
/// ⚠️ une mesure de « glissement » dont les bornes ne bougent pas (repère vertical attrapé, glissement refusé)
/// ressemblerait à un gain : le banc panique au lieu de la publier (esprit de L-071).
fn cadres_glissement(h: &mut Harness<'_, App>, n: usize) -> Vec<f64> {
    /// Déplacement du pointeur d'un cadre au suivant.
    const PAS_PX: f32 = 8.0;
    /// Écart maximal au point de départ, avant de repartir dans l'autre sens.
    const AMPLITUDE_PX: f32 = 80.0;

    let tr = h.state().viewer.transforms[0].expect("le graphique Roll a été dessiné");
    let frame = tr.frame();
    // loin des repères verticaux (début et fin de période aux bords, curseur), et hors du mode « Mesurer »
    let depart = egui::pos2(frame.left() + frame.width() * 0.6, frame.center().y);
    let bornes = |h: &Harness<'_, App>| {
        let b = *h.state().viewer.transforms[0].expect("graphique dessiné").bounds();
        (b.min()[0], b.max()[0])
    };

    h.hover_at(depart);
    h.step();
    h.drag_at(depart);
    h.step();

    let mut sens = 1.0f32;
    let mut ecart = 0.0f32;
    let mut avance = |i: usize| {
        if ecart.abs() >= AMPLITUDE_PX {
            sens = -sens;
        }
        ecart += sens * PAS_PX;
        let _ = i;
        egui::pos2(depart.x + ecart, depart.y)
    };
    for i in 0..CHAUFFE {
        h.hover_at(avance(i));
        h.step();
    }
    let mut bouge = 0usize;
    let ms: Vec<f64> = (0..n)
        .map(|i| {
            let avant = bornes(h);
            h.hover_at(avance(i));
            let t0 = Instant::now();
            h.step();
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            if bornes(h) != avant {
                bouge += 1;
            }
            ms
        })
        .collect();
    h.drop_at(egui::pos2(depart.x + ecart, depart.y));
    h.step();
    assert!(
        bouge * 10 >= n * 9,
        "glissement immobile : bornes changées sur {bouge} cadres sur {n} (repère vertical attrapé ?)"
    );
    ms
}

/// Pic de mémoire du processus (`VmHWM` de Linux), en Mo ; `None` ailleurs.
fn pic_memoire_mo() -> Option<f64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let ligne = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    let ko: f64 = ligne.split_whitespace().nth(1)?.parse().ok()?;
    Some(ko / 1024.0)
}

/// Nombre d'échantillons de la session affichée qui tombent dans la fenêtre du graphique Roll.
fn visibles(h: &Harness<'_, App>, i: usize) -> usize {
    let b = *h.state().viewer.transforms[0].expect("graphique dessiné").bounds();
    let (x0, x1) = (b.min()[0], b.max()[0]);
    let t = &h.state().state.logs[i].log.t;
    t.partition_point(|&v| v <= x1) - t.partition_point(|&v| v < x0)
}

/// Zoome la vue sur environ `1 / fraction` du log, par des évènements de zoom au centre du graphique Roll (ce que
/// fait Ctrl + molette dans l'application). Rend le nombre d'échantillons visibles obtenu.
fn zoomer(h: &mut Harness<'_, App>, i: usize, fraction: f64) -> usize {
    let centre = h.state().viewer.transforms[0]
        .expect("graphique dessiné")
        .frame()
        .center();
    let depart = visibles(h, i) as f64;
    for _ in 0..200 {
        if (visibles(h, i) as f64) <= depart / fraction {
            break;
        }
        h.hover_at(centre);
        h.step();
        h.event(egui::Event::Zoom(1.2));
        h.step();
    }
    let obtenu = visibles(h, i);
    assert!(
        (obtenu as f64) <= depart / (fraction * 0.6),
        "zoom insuffisant : {obtenu} échantillons visibles au lieu de {:.0}",
        depart / fraction
    );
    obtenu
}

/// Réglage `wgpu` qui prend une vraie carte graphique : par défaut, `egui_kittest` **préfère le rendu logiciel** (llvmpipe),
/// qui ferait mesurer le processeur à la place de la carte (≈ 44 ms au lieu de 0,3 ms sur `FINAL.bbl`). Ordre : carte
/// dédiée, carte intégrée, autre, rendu logiciel en dernier recours (annoncé). La carte retenue est affichée.
// ⚠️ sans ce choix, `--gpu` mesure le rendu logiciel, donc le processeur (L-084)
fn vraie_carte() -> eframe::egui_wgpu::WgpuSetup {
    use eframe::egui_wgpu::{WgpuSetup, wgpu::DeviceType};
    let WgpuSetup::CreateNew(mut setup) = egui_kittest::wgpu::default_wgpu_setup() else {
        unreachable!("egui_kittest crée toujours son propre périphérique")
    };
    setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, _surface| {
        let rang = |t: DeviceType| match t {
            DeviceType::DiscreteGpu => 0,
            DeviceType::IntegratedGpu => 1,
            DeviceType::Other | DeviceType::VirtualGpu => 2,
            DeviceType::Cpu => 3,
        };
        let a = adapters
            .iter()
            .min_by_key(|a| rang(a.get_info().device_type))
            .ok_or("aucune carte graphique")?;
        let info = a.get_info();
        let logiciel = if info.device_type == DeviceType::Cpu {
            " — RENDU LOGICIEL : la mesure est celle du processeur"
        } else {
            ""
        };
        println!(
            "Carte : {} ({:?}, {:?}){logiciel}",
            info.name, info.backend, info.device_type
        );
        Ok(a.clone())
    }));
    WgpuSetup::CreateNew(setup)
}

fn main() {
    let o = options();
    // `App::new`, jamais `App::from_launch` : la mesure ne doit pas dépendre des préférences de celui qui la lance
    let mut b = Harness::builder()
        .with_size(egui::vec2(o.largeur, 1080.0))
        .with_context_setup(otb_gui::app::setup); // L-066
    if o.gpu {
        b = b.wgpu_setup(vraie_carte());
    }
    let mut h = b.build_ui_state(|ui, a: &mut App| a.ui(ui), App::new(otb_gui::tools::all_pages()));

    // ouverture en arrière-plan : on attend une première session, puis le calme
    h.state_mut().open(vec![o.log.clone()]);
    let debut = Instant::now();
    let (mut vues, mut depuis) = (0, Instant::now());
    loop {
        h.step();
        let n = h.state().state.logs.len();
        if n != vues {
            (vues, depuis) = (n, Instant::now());
        }
        if n > 0 && depuis.elapsed() >= CALME {
            break;
        }
        assert!(debut.elapsed() < LIMITE, "ouverture jamais terminée");
        std::thread::sleep(Duration::from_millis(10));
    }

    // la session la plus longue : c'est elle qui coûte
    let (i, n) = h
        .state()
        .state
        .logs
        .iter()
        .map(|l| l.log.t.len())
        .enumerate()
        .max_by_key(|&(_, n)| n)
        .unwrap();
    h.state_mut().state.set_current(Some(i));
    let nom = h.state().state.logs[i].label.clone();
    println!(
        "{} : {} session(s), mesurée : « {nom} », {n} échantillons ; fenêtre {} × 1080, {} cadres (médiane), {CHAUFFE} de chauffe",
        o.log.display(),
        vues,
        o.largeur,
        o.cadres
    );

    // écran Guide : on laisse finir sa vérification du log avant de mesurer
    h.state_mut().screen = Screen::Guide;
    let debut = Instant::now();
    loop {
        h.step();
        if !h.state().guide.busy() {
            break;
        }
        assert!(debut.elapsed() < LIMITE, "guide jamais prêt");
        std::thread::sleep(Duration::from_millis(10));
    }
    let (g_min, g_med) = resume(cadres(&mut h, o.cadres));

    // viewer : un changement de log recadre sur la vue entière (`ViewerPage::ui`)
    h.state_mut().screen = Screen::Viewer;
    let (v_min, v_med) = resume(cadres(&mut h, o.cadres));
    assert_eq!(
        h.state().screen,
        Screen::Viewer,
        "le viewer a été quitté pendant la mesure"
    );

    println!("{:<28} {:>9} {:>9}", "", "min (ms)", "méd (ms)");
    println!("{:<28} {g_min:>9.3} {g_med:>9.3}", "Guide (aucune courbe)");
    println!("{:<28} {v_min:>9.3} {v_med:>9.3}", "Viewer, vue entière");
    let d = v_med - g_med;
    println!("{:<28} {:>9.3} {d:>9.3}", "Viewer − Guide", v_min - g_min);
    println!("Coût par échantillon (médianes) : {:.1} ns", d * 1e6 / n as f64);

    // mémoire : ce que pèsent les courbes en cache et leurs index, face au pic du processus
    let (valeurs, index) = h.state().viewer.cache_bytes();
    let mo = |o: usize| o as f64 / (1024.0 * 1024.0);
    match pic_memoire_mo() {
        Some(pic) => println!(
            "Mémoire : courbes en cache {:.1} Mo, index {:.1} Mo ({:.0} % des courbes, {:.1} % du pic) ; pic du processus {pic:.0} Mo",
            mo(valeurs),
            mo(index),
            100.0 * index as f64 / valeurs.max(1) as f64,
            100.0 * mo(index) / pic
        ),
        None => println!(
            "Mémoire : courbes en cache {:.1} Mo, index {:.1} Mo",
            mo(valeurs),
            mo(index)
        ),
    }

    // tessellation (formes → triangles) du cadre du viewer : travail du processeur, hors de `step`, que la vraie
    // application paie aussi avant d'envoyer le cadre à la carte
    let ms: Vec<f64> = (0..CHAUFFE + o.cadres)
        .map(|_| {
            h.step();
            let (formes, ppp) = (h.output().shapes.clone(), h.output().pixels_per_point);
            let t0 = Instant::now();
            let prims = h.ctx.tessellate(formes, ppp);
            let ms = t0.elapsed().as_secs_f64() * 1e3;
            drop(prims);
            ms
        })
        .skip(CHAUFFE)
        .collect();
    let (t_min, t_med) = resume(ms);
    println!("{:<28} {t_min:>9.3} {t_med:>9.3}", "Tessellation du viewer");

    if o.zoom > 1.0 {
        let vus = zoomer(&mut h, i, o.zoom);
        let (z_min, z_med) = resume(cadres(&mut h, o.cadres));
        println!(
            "{:<28} {z_min:>9.3} {z_med:>9.3}   ({vus} échantillons visibles)",
            "Viewer, vue zoomée"
        );
        println!("{:<28} {:>9.3} {:>9.3}", "Zoomée − Guide", z_min - g_min, z_med - g_med);
        // retour à la vue entière pour la suite des mesures
        h.state_mut().viewer.fit_all();
        h.step();
    }

    if o.glissement {
        let (d_min, d_med) = resume(cadres_glissement(&mut h, o.cadres));
        println!("{:<28} {d_min:>9.3} {d_med:>9.3}", "Viewer, glissement");
        println!(
            "{:<28} {:>9.3} {:>9.3}",
            "Glissement − Guide",
            d_min - g_min,
            d_med - g_med
        );
        println!(
            "Coût par échantillon pendant le glissement (médianes) : {:.1} ns",
            (d_med - g_med) * 1e6 / n as f64
        );
    }

    if o.gpu {
        // `render` ne refait rien sur un cadre déjà rendu, d'où un `step` avant chacun ; il refait la tessellation, crée
        // une texture et relit l'image, un coût fixe qui ne dépend pas des courbes : la différence avec l'écran Guide l'ôte
        let rendu = |h: &mut Harness<'_, App>| {
            let ms: Vec<f64> = (0..CHAUFFE + o.cadres)
                .map(|_| {
                    h.step();
                    let t0 = Instant::now();
                    h.render().expect("rendu wgpu");
                    t0.elapsed().as_secs_f64() * 1e3
                })
                .skip(CHAUFFE)
                .collect();
            resume(ms)
        };
        let (rv_min, rv_med) = rendu(&mut h);
        h.state_mut().screen = Screen::Guide;
        let (rg_min, rg_med) = rendu(&mut h);
        println!(
            "{:<28} {rg_min:>9.3} {rg_med:>9.3}   (texture et relecture de l'image comprises)",
            "Rendu wgpu du Guide"
        );
        println!(
            "{:<28} {rv_min:>9.3} {rv_med:>9.3}   (idem, tessellation comprise)",
            "Rendu wgpu du viewer"
        );
        println!(
            "{:<28} {:>9.3} {:>9.3}   (majorant de la carte graphique pour les courbes)",
            "Rendu viewer − Guide",
            rv_min - rg_min,
            rv_med - rg_med
        );
    }
}
