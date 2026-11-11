<!-- Titre de la PR : une phrase en français qui dit l'effet du changement (ex. « Le programme ne plante plus à la fermeture sous Windows ») ;
     elle devient la ligne des notes de version. Une seule étiquette « RN: … » (tableau dans CONTRIBUTING.md, D-056). -->

## Objet

<!-- Ce que change cette demande, et pourquoi. -->

## Registres

<!-- Numéros créés ou modifiés : D-xxx (docs/decisions.md), L-xxx (docs/lecons.md), W-xxx (docs/travaux-reportes.md). « Aucun » sinon. -->

## Vérifications

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` et `cargo test` (dans `rust/`) passent
- [ ] Testé sur de vrais logs (lesquels ?) :
- [ ] Documentation à jour : documents relus selon `.claude/skills/docs-sync` (README, guide, AGENTS, docs/…)
- [ ] Titre en phrase et une étiquette `RN: …` (`RN: ignorer` si rien ne change pour le pilote)
- [ ] Registres complétés si besoin (décision, leçon, travail reporté)
- [ ] Captures avant / après jointes si l'affichage change
