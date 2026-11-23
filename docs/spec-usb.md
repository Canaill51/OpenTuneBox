# Récupération des logs par USB : ce que fait le firmware

Vérification des **huit points** que le lot 32 ([plan.md](plan.md)) exige **avant d'écrire une ligne de code** : l'étude
du 2026-11-14 avait été rédigée de mémoire. Tout ce qui suit a été relu le 2026-11-22 dans les sources de Betaflight
(`master` au commit `1b94a3871` du 2026-11-21) et de son configurateur (`master` au commit `663c6cc6` du 2026-11-20).
Le dépôt cloné n'a pas d'étiquettes de version : les dates d'apparition viennent de `git log`. **Rien n'est copié**
de ces sources (GPL 3, D-004) : on n'en garde que le protocole, c'est-à-dire ce qui passe sur le fil.

Les chemins de fichiers ci-dessous sont ceux du dépôt Betaflight (`src/main/…`) ou de son configurateur (`src/…`).

## Résumé

| # | Point | Réponse | Sûr ? |
|---|---|---|---|
| 1 | Le mode stockage de masse (MSC) lit-il la mémoire interne ? | Oui depuis avril 2018, **en lecture seule** | lu dans le code |
| 2 | Débit réel de la lecture MSP | blocs de 4 096 octets au plus ; **3,25 Mo par minute** mesurés (≈ 5 min pour 16 Mo) | mesuré sur une carte (2026-11-22) |
| 3 | Durée d'un effacement de 16 Mo, et comment savoir qu'il est fini | drapeau « prête » du résumé MSP ; jusqu'à 200 s par tranche de 16 Mo ; **42,5 s** mesurées | fin : lue ; durée : mesurée sur une carte (2026-11-22) |
| 4 | Identifiant de la carte | `MSP_UID` : 96 bits gravés dans le microcontrôleur, **0 sur deux cas** | lu dans le code |
| 5 | Effacement d'une carte SD | **impossible en MSP** ; par le système, une fois la carte montée en MSC | lu dans le code |
| 6 | Bibliothèque série côté Rust | `serial2` (BSD-2 ou Apache-2.0) ; `serialport` **refusée** (MPL-2.0) | licences vérifiées, `cargo deny` vert (lot 32a) |
| 7 | Autres firmwares qui parlent MSP | hors périmètre (D-062) : refuser tout ce qui n'est pas `BTFL` | décision existante |
| 8 | Appliquer les réglages « Et si » sur la carte | possible techniquement ; **à trancher par Cédric** | question ouverte |

Deux constats **changent le dessin du lot 32** :

- ⚠️ **Le firmware ne protège ni la lecture ni l'effacement quand le drone est armé.** Seul le redémarrage est refusé
  (`src/main/msp/msp_reboot.h:mspRebootIsAllowed`). OpenTuneBox doit donc relire l'état armé **juste avant**
  l'effacement, pas seulement au branchement.
- ⚠️ **En mode stockage de masse, la mémoire interne est en lecture seule.** Copier par MSC puis effacer demande de
  redémarrer la carte en mode normal et d'envoyer l'effacement par MSP : deux connexions, donc deux identifications
  de la carte, qui doivent donner le même identifiant.

## 1. Mode stockage de masse

- **Redémarrer en MSC** : `MSP_REBOOT` (68) avec un octet de mode — 0 firmware, 1 chargeur ROM, **2 MSC**, **3 MSC avec
  l'heure UTC**, 4 chargeur en mémoire flash (`src/main/msp/msp.c`, énumération en tête). Le mode 2 règle l'horloge
  des fichiers sur le fuseau `tz_offset_minutes`, le mode 3 sur UTC. Refusé si le drone est armé. Ajouté le
  **2018-07-15** (`df55fc99e`).
- **Compilé seulement si** `USE_USB_MSC` est défini : microcontrôleurs STM32 F4 (F411 compris), F7 et H7, et les familles
  AT32, APM32, GD32 ; **retiré** sur les cibles ESP32, sans port USB virtuel, sans blackbox, ou sans mémoire ni carte SD
  (`src/main/target/common_post.h`). Une carte sans MSC n'a que la voie MSP.
- **Mémoire interne vue comme un disque** depuis le **2018-04-11** (`d749879cf`, émulation FAT de `src/main/msc/emfat_file.c`) :
  un fichier `BTFL_ALL.BBL` (toute la mémoire) et un fichier `BTFL_001.BBL`, `BTFL_002.BBL`… par log. Le firmware découpe en
  cherchant l'en-tête `H Product:Blackbox` tous les 2 048 octets. **Disque protégé en écriture**
  (`src/main/msc/usbd_storage_emfat.c:STORAGE_IsWriteProtected` rend 1) : on ne peut rien y effacer.
- **Carte SD vue comme un disque** : c'est sa vraie FAT, inscriptible en SDIO (`src/main/msc/usbd_storage_sdio.c` rend 0).
- **Sortie du mode MSC** : débrancher ou redémarrer la carte ; il n'y a plus de port série pendant ce temps. La carte
  tourne en boucle sur le disque (`src/main/drivers/usb_msc_common.c:mscWaitForButton`) : seule une coupure
  d'alimentation (USB **et** batterie) ou le bouton MSC, s'il est câblé, la ramène au firmware.
- **Réponse au redémarrage** (lu le 2026-11-23, lot 32c) : `MSP_REBOOT` rend le mode accepté, puis, pour le mode 2
  seulement, un octet « disque prêt » ; **pas prêt, la carte ne redémarre pas**. Prêt veut dire : le support désigné par
  `blackbox_device` est utilisable (`src/main/io/usb_msc.c:mscCheckFilesystemReady`) — le disque montre **ce** support, pas
  l'autre. Refus (`!`) : drone armé, ou firmware compilé sans `USE_USB_MSC`. OpenTuneBox n'envoie que le mode 2 (D-075).
- **`BTFL_ALL.BBL` a la taille de la place occupée** (`flashfsIdentifyStartOfFreeSpace`, la valeur que rend
  `MSP_DATAFLASH_SUMMARY`) : c'est, octet pour octet, la copie MSP, et elle passe par les mêmes contrôles. Il n'existe que
  si la mémoire contient au moins un log. Le disque se présente comme `BETAFLT` / `Onboard Flash` (mémoire interne) ou
  `STM` (carte SD) (`src/main/msc/usbd_storage_*.c`).

## 2. Lecture bloc par bloc en MSP

- **Requête** `MSP_DATAFLASH_READ` (71) : adresse (u32), longueur voulue (u16), compression permise (u8, facultatif).
  Sans la longueur, le firmware passe à l'**ancien format** de 128 octets complétés par des zéros : à ne jamais utiliser
  (`src/main/msp/msp.c:mspFcDataFlashReadCommand`).
- **Réponse** : adresse (u32), longueur réellement lue (u16), méthode de compression (u8 : 0 aucune, 1 Huffman), puis les
  données. La longueur est **plus courte** que demandée en fin de mémoire, et au plus 4 096 octets moins 16
  (`MSP_PORT_DATAFLASH_BUFFER_SIZE`, `src/main/msp/msp_serial.h`).
- **Trame** : une réponse de plus de 254 octets passe en **trame « jumbo »** de MSP v1 (taille 16 bits), ou en MSP v2.
  Le décodeur de trames d'OpenTuneBox doit savoir lire les deux.
- **Compression Huffman** : facultative ; ne pas la demander au premier jet (une table de plus à réimplémenter d'après
  le protocole, pour un gain qui reste à mesurer).
- **Débit** : le configurateur lit par blocs de 4 096 (`src/components/tabs/OnboardLoggingTab.vue`, `BLOCK_SIZE`) et
  affiche le débit obtenu dans sa console, sans valeur de référence. **À mesurer sur une vraie carte** avant de promettre
  une durée à l'écran ; l'étude parlait de « plusieurs minutes pour 16 Mo », chiffre jamais vérifié.
- **Le configurateur ne propose d'effacer que si la taille téléchargée atteint la place occupée annoncée** (même fichier,
  `conditionallyEraseFlash`) : c'est le premier des contrôles exigés par le lot 32, OpenTuneBox en ajoute trois.

## 3. Effacement de la mémoire interne

- **Requête** `MSP_DATAFLASH_ERASE` (72), sans contenu. Le firmware arrête l'enregistrement puis lance l'effacement
  (`src/main/blackbox/blackbox_io.c:blackboxEraseAll`) : effacement **de toute la puce** si une seule partition couvre
  toute la mémoire NOR, sinon **secteur par secteur en tâche de fond** (`src/main/io/flashfs.c:flashfsEraseCompletely`).
  La réponse arrive **avant** la fin de l'effacement.
- **Savoir qu'il est fini** : `MSP_DATAFLASH_SUMMARY` (70) rend drapeaux (u8 : bit 0 « prête », bit 1 « présente »),
  nombre de secteurs (u32), taille totale (u32), **place occupée** (u32). L'effacement est fini quand le bit « prête »
  revient à 1 et que la place occupée vaut 0.
- **Durée** : le configurateur interroge toutes les 500 ms et abandonne après **200 s par tranche de 16 Mo, plus 100 s**
  (`src/composables/useDataflashErase.ts`) — c'est le pire cas d'une puce NOR, pas une durée typique. À mesurer sur une
  vraie carte ; l'écran montre une attente sans durée promise.
- ⚠️ **Aucun contrôle de l'état armé** dans ce chemin du firmware (voir le résumé).

## 4. Identité de la carte

- **Identifiant** : `MSP_UID` (160) rend trois u32, l'identifiant unique **gravé à la fabrication** du microcontrôleur
  (STM32 `UID_BASE` ; adresses fixes pour AT32, APM32, GD32, CH32 ; lu au démarrage sur ESP32 et RP2350). Il ne change ni
  avec le firmware, ni avec la configuration : seulement si on change de contrôleur de vol. Existe depuis 2014.
- ⚠️ **Il vaut 0** sur le cœur M4 des H7 double cœur et sur le simulateur SITL (`src/platform/STM32/include/platform/platform.h`).
  Un identifiant nul ne sert jamais de clé : la carte est alors traitée comme inconnue, et le pilote choisit le drone.
- **Le reste de l'identité** : `MSP_FC_VARIANT` (2, quatre lettres, `BTFL` pour Betaflight), `MSP_FC_VERSION` (3, trois
  octets), `MSP_BOARD_INFO` (4, nom de la cible), `MSP_NAME` (10, le `craft_name`, que le pilote change d'une commande),
  `MSP_API_VERSION` (1, à demander **en premier**, comme le recommande `src/main/msp/msp_protocol.h`).
- **État armé** : `MSP_STATUS_EX` (150) ; ses 32 premiers bits de modes portent l'armement au **bit 0**, parce que le mode
  `ARM` est toujours actif et toujours le premier (`src/main/msp/msp_box.c:packFlightModeFlags`).

## 5. Carte SD

`blackboxEraseAll` ne fait rien hors de la mémoire interne : **l'effacement MSP n'existe pas pour une carte SD.** Ses
logs se récupèrent par le mode stockage de masse (point 1), et s'effacent comme des fichiers ordinaires, par le système,
avec les mêmes contrôles avant (lot 32). `MSP_SDCARD_SUMMARY` (79) donne l'état de la carte et sa place libre : drapeaux
(bit 0 : carte prévue), état (0 absente, 1 en panne, 2 et 3 en préparation, **4 prête**), dernière erreur, place libre puis
totale en Ko (`serializeSDCardSummaryReply`). Les logs sont dans **`LOGS/LOG00001.BFL`**, `LOG00002.BFL`…
(`src/main/blackbox/blackbox_io.c` : `LOGFILE_PREFIX`, `LOGFILE_SUFFIX`, dossier créé par `afatfs_mkdir("logs")`).
OpenTuneBox les copie et vérifie un par un, puis ne supprime que ceux-là, chacun relu et comparé à sa copie juste avant
(`rust/otb-usb/src/sd.rs:delete_sd_logs`, D-075).

## 6. Bibliothèque série

| Crate | Version (2026-11-22) | Licence | Verdict |
|---|---|---|---|
| `serialport` | 4.10.1 | MPL-2.0 | **refusée** : copyleft faible, absente de `rust/deny.toml` (D-043, D-045) |
| `serial2` | 0.2.39 | BSD-2-Clause OR Apache-2.0 | **retenue** : ses dépendances (`libc`, `cfg-if`, `windows-sys`) passent `cargo deny`, vérifié au lot 32a (2026-11-22) |

`serial2` liste les ports sans donner l'identifiant USB du fabricant : un port se reconnaît en lui demandant
`MSP_API_VERSION`, ce qui évite aussi de dépendre de la marque du contrôleur. Les droits sur le port sous Linux
(groupe `dialout`) et le pilote du port virtuel sous Windows restent à détecter et à expliquer, comme le dit le lot 32 ;
leur détail sera vérifié sur les deux postes de Cédric, pas de mémoire.

## 7. Autres firmwares

D-062 limite le projet à Betaflight. Si `MSP_FC_VARIANT` ne rend pas `BTFL`, OpenTuneBox s'arrête avec un message clair
avant toute lecture ou effacement : d'autres firmwares parlent un MSP voisin, avec des numéros de messages qui peuvent
différer, et aucun de leurs logs n'est lu (W-001).

## 8. Réglages « Et si » appliqués sur la carte

Possible techniquement (le CLI accepte des `set …` puis `save`, ou les messages MSP de réglage) ; **ce n'est pas une
question technique** : écrire sur la carte d'un drone change sa façon de voler. À trancher par Cédric (plan, lot 32).

## Sauvegarde de la configuration (`diff all`)

Même port : un caractère `#` fait entrer dans le CLI, `diff all` renvoie la configuration, la fin de la réponse se reconnaît
à l'invite `# `. Sortir par `exit` **redémarre la carte** ; `exit noreboot` existe depuis le **2024-09-26** (`331801c7a`).
Sur une version plus ancienne, le redémarrage ferme le port : la sauvegarde se fait donc **en dernier**, après la copie et
l'éventuel effacement.

## Procédure d'essai à la main

Aucune carte en CI : `otb-usb` (lot 32a) est testé sur une carte simulée et, sous Linux, par un pseudo-terminal. Ces
essais se font sur une vraie carte, résultats notés ici avec la date et la carte. Depuis `rust/`, carte branchée,
**hélices retirées**, fichiers écrits **hors du dépôt** (jamais dans `$OTB_LOGS`) :

```sh
cargo run -p otb-dev -- usb ports                          # ports où une carte répond en MSP
cargo run -p otb-dev -- usb info /dev/ttyACM0              # identité, état armé, mémoire (COM3… sous Windows)
cargo run --release -p otb-dev -- usb copy /dev/ttyACM0 ~/Documents/Drones/usb            # copie, contrôles, débit (dossier créé s'il manque)
cargo run --release -p otb-dev -- usb copy /dev/ttyACM0 ~/Documents/Drones/usb --erase    # idem, puis effacement après avoir tapé « effacer »
cargo run -p otb-dev -- usb diff /dev/ttyACM0 ~/Documents/Drones/usb/config.txt  # `diff all` ; la carte redémarre ensuite
cargo run --release -p otb-dev -- usb msc /dev/ttyACM0 ~/Documents/Drones/usb [--erase]  # mode stockage de masse : copie, contrôles, débit ; --erase : rebrancher, puis effacer en MSP (mémoire) ou supprimer les logs vérifiés (SD)
cargo run --release -p otb-dev -- usb sd /media/$USER/CARTE ~/Documents/Drones/usb [--erase]  # carte SD dans un lecteur de cartes (racine du disque monté)
```

- [x] Débit de la lecture MSP par blocs de 4 096 octets (Mo par minute), sur une mémoire pleine (`usb copy`) : 3,25 Mo par minute.
- [x] Durée d'un effacement complet, et place occupée relue à 0 ensuite (`usb copy … --erase`) : 42,5 s, relue à 0.
- [ ] `MSP_UID` identique avant et après un nouveau firmware, et après redémarrage en MSC puis retour (`usb info` en montre
      une empreinte courte, pas le numéro : ne jamais recopier l'identifiant complet d'une carte dans le dépôt, public).
- [ ] Mode stockage de masse : fichiers vus sous Linux et Windows, contenu identique à la lecture MSP (`usb msc` puis
      `usb copy` sur la même mémoire : mêmes octets). Disque monté seul ou à ouvrir dans le gestionnaire de fichiers ?
      Débit de la copie par le disque (Mo par minute).
- [ ] `usb msc … --erase` sur la mémoire interne : après rebranchement, l'effacement passe (même identifiant) et la place
      relue vaut 0.
- [ ] Carte SD (dans un drone, `usb msc`, et dans un lecteur, `usb sd`) : logs `LOGS/*.BFL` trouvés, copiés et vérifiés ;
      `--erase` ne supprime qu'eux ; la carte se relit ensuite dans le drone (nouveau log enregistré).
- [ ] Copie relue dans OpenTuneBox : mêmes vols que le fichier téléchargé par le configurateur.
- [ ] `usb diff` : même texte que `diff all` tapé dans le CLI du configurateur (forme déjà vérifiée, voir les résultats).
- [ ] Port déjà ouvert : configurateur Betaflight connecté, `usb info` doit dire le port occupé (Linux : le verrou
      d'OpenTuneBox n'arrête que les programmes qui le prennent aussi ; **supposé** que le configurateur ouvre le port
      en exclusif, à vérifier) ; Windows : message « déjà ouvert par un autre programme ».
- [ ] Drone armé (sur batterie, hélices retirées) : `usb copy … --erase` refuse d'effacer.
- [x] **L'écran « Récupérer par USB »** (lot 32b) : `cargo run --release -p otb-gui`, tableau de bord, **Récupérer par
      USB**, **Se connecter**. Première connexion : choisir le drone, puis **Copier les vols** (barre de progression,
      temps restant proche du débit mesuré) ; la copie apparaît sur la carte du drone ; **Effacer la mémoire du
      drone…** puis **Effacer définitivement** ; **Relire l'état** : mémoire à 0, drone reconnu sans question.
      **Sauvegarder la configuration** deux fois de suite : la seconde dit « identique ». Drone armé : boutons grisés.

Choix de `otb-usb` pour la sortie du CLI : `exit` (la carte redémarre), et non `exit noreboot`, absent avant 2024-09 et qui
laisse l'armement bloqué (`ARMING_DISABLED_CLI`) jusqu'au prochain redémarrage.

### Résultats

**2026-11-22, Linux, carte F722 (STM32F722), Betaflight 2026.6.2, API MSP 1.48**, branchée seule en USB :

- `usb ports` : la carte répond sur `/dev/ttyACM0`, ouvert sans réglage de droits (compte déjà dans `dialout`).
- `usb info` : identité lue (`F722`, `STM32F722`, nom vide), identifiant non nul, non
  armée, mémoire de 8 Mo (128 secteurs) **vide** — débit de copie et durée d'effacement donc **pas encore mesurés**.
- `usb copy` : mémoire vide, copie refusée comme prévu (« rien à copier », effacement refusé).
- `usb diff` : 62 lignes, de `# version` à `save`, sans écho de la commande ni invite du CLI ; la carte a redémarré.

**2026-11-22, Linux, carte G474 (STM32G474), Betaflight 2026.12.0, API MSP 1.49**, mémoire de 16 Mo (256 secteurs), nom
`AIR65 CED`, identifiant non nul, non armée :

- `usb copy … --erase` : **8 187 904 octets copiés en 144,2 s, soit 3,25 Mo par minute** (≈ 57 Ko/s, environ 70 ms par
  bloc de 4 080 octets). Une mémoire de 16 Mo pleine prendrait donc **environ 5 minutes** : l'écran du lot 32b doit
  montrer une progression, pas un sablier. Contrôles passés : 1 log, 1 fragment de vol ; le fichier relu par
  `opentunebox info` donne un vol de 45,1 s enregistré à 4 kHz (debug `FFT_FREQ`) ; ses 1 469 derniers octets valent `0xFF`
  (mémoire vierge comprise dans la place annoncée), sans gêne pour le décodeur.
- Effacement confirmé en tapant « effacer » : **42,5 s**, place occupée relue à **0** ensuite — loin des 300 s du délai
  du configurateur pour 16 Mo.
- Pas encore essayé : refus d'effacer drone armé, port tenu par le configurateur, identifiant après changement de
  firmware, stockage de masse, Windows.

**2026-11-23, Linux, même carte G474** — l'écran « Récupérer par USB » (lot 32b), par Cédric, suivant la procédure
ci-dessus : connexion, choix du drone, copie, ouverture des vols, effacement, relecture (drone reconnu sans question),
deux sauvegardes de la configuration — **tout s'est passé comme prévu** (« extrêmement bien »). Durées et essais
facultatifs (drone armé, port occupé) non relevés.

**2026-11-23, Linux, même carte G474** — `usb msc` (lot 32c-1), par Cédric : la carte redémarre en mode stockage de
masse, le disque apparaît **monté par le système, sous le nom du lecteur**, et la copie se fait. Débit non relevé ;
`--erase` après rebranchement, Windows et **carte SD** pas encore essayés (aucune carte de vol à carte SD sous la main).

