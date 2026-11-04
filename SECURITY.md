# Politique de sécurité

## Signaler une faille

Ne pas ouvrir de ticket public pour une faille de sécurité. Utiliser le signalement privé de GitHub :
onglet **Security** du dépôt, puis **Report a vulnerability**. Le message n'est vu que par le mainteneur.

Merci d'indiquer :
- la version d'OpenTuneBox (celle de la page Releases d'où vient le fichier) et le système (Windows ou Linux) ;
- ce qui se passe et comment le reproduire, avec si possible un petit log Blackbox qui déclenche le problème
  (sans donnée personnelle : un log contient le nom du drone) ;
- l'effet que vous craignez.

Il s'agit d'un projet personnel tenu sur le temps libre : pas de délai garanti, mais chaque signalement est lu.
Pas de prime de découverte.

## Ce qui compte comme faille

OpenTuneBox est conçu pour fonctionner **hors ligne** : l'assistant n'appelle aucune API et n'envoie aucune donnée (D-003 dans `docs/decisions.md`). Les risques les plus probables sont :
- un fichier `.bbl` ou `.bfl` malformé qui fait planter le décodeur ou lui fait lire ou écrire hors de ses données ;
- une faille dans une bibliothèque embarquée (voir `THIRD-PARTY-LICENSES.txt` pour la liste) ;
- un défaut de la chaîne de publication (workflows GitHub, installateurs).

Un plantage sur un log valide est un défaut ordinaire : à signaler par un ticket normal.

## Versions prises en charge

Seule la dernière version publiée reçoit des correctifs.
