<!-- Titre de la PR : exactement le nom de la branche (ex. fix/windows-close-crash). -->

## Objet

<!-- Ce que change cette demande, et pourquoi. -->

## Registres

<!-- Numéros créés ou modifiés : D-xxx (docs/decisions.md), L-xxx (docs/lecons.md), W-xxx (docs/travaux-reportes.md). « Aucun » sinon. -->

## Vérifications

- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` et `cargo test` (dans `rust/`) passent
- [ ] Testé sur de vrais logs (lesquels ?) :
- [ ] Documentation à jour : documents relus selon `.claude/skills/docs-sync` (README, guide, AGENTS, docs/…)
- [ ] `CHANGELOG.md` mis à jour (section *Non publié*)
- [ ] Registres complétés si besoin (décision, leçon, travail reporté)
- [ ] Captures avant / après jointes si l'affichage change
