# Prompt : test silicium OASIS sur 3 Raspberry Pi Pico

À coller tel quel dans Claude Code, lancé **sur le PC où les 3 cartes sont branchées**,
à la racine du dépôt (`c:\dev\oasis`).

---

## Contexte

Tu travailles sur OASIS, un runtime Rust `no_std` (crate `oasis-rt`). Lis d'abord
`CLAUDE.md`, `IOT_READINESS.md` et `OASIS_POC_GUIDE.html`. Aujourd'hui, **rien n'a
jamais tourné sur du silicium réel** : tout a été fait sous Wokwi (RP2040), sous
Renode (STM32F4) ou sur x86. Ta mission est de produire la première preuve
matérielle, sur 3 cartes branchées en USB à ce PC.

Le résultat sera montré à des ingénieurs embarqués. Une preuve fausse ou embellie
est pire que pas de preuve.

## Règles non négociables

1. **Aucune donnée inventée.** Chaque chiffre de ton rapport doit venir d'un log
   brut capturé sur la carte et archivé. Si une mesure échoue, écris « ÉCHEC » et
   la cause. Ne la remplace jamais par une estimation ou un chiffre de simulation.
2. **Ne flashe que des cartes identifiées.** Ne copie jamais un fichier sur un
   lecteur qui n'est pas un volume `RPI-RP2` ou `RP2350`, ne formate rien, et
   n'écris sur aucun autre disque. En cas de doute, arrête-toi et demande-moi.
3. **N'appelle pas « réussi » un test qui ne prouve rien.** Un compteur qui ne
   bouge pas, un `grep` qui renvoie « Binary file matches », un résultat lu sur le
   PC et non sur la carte ne sont pas des preuves.
4. **Le driver SX1262 est un stub.** N'affirme rien sur la radio LoRa tant qu'un
   vrai driver n'a pas émis et reçu un paquet entre deux cartes.
5. Garde les 443 tests verts : `cargo test --workspace --release` avant et après
   tes modifications.

## Étape 0 : identifier le matériel (lecture seule)

- Liste les périphériques USB et les ports série (Windows : `Get-PnpDevice -PresentOnly`,
  `Get-CimInstance Win32_SerialPort`, `mode`; Linux : `lsusb`, `ls /dev/ttyACM*`).
- Détermine le modèle exact de chaque carte, à partir des VID:PID :
  - `2E8A:0003` = RP2040 en mode BOOTSEL, donc Pico ou Pico W ;
  - `2E8A:000F` = RP2350 en mode BOOTSEL, donc Pico 2, et la cible devient `thumbv8m.main-none-eabihf` ;
  - `2E8A:000C` = Debug Probe (CMSIS-DAP) ;
  - si ce sont des Raspberry Pi 4 ou 5 (Linux complet), **arrête-toi et demande-moi** :
    le plan est différent (cross-compilation `aarch64`, SSH, pas de flash).
- Si une carte n'apparaît pas, demande-moi de la rebrancher en maintenant BOOTSEL.
- Donne à chaque carte un nom stable (A, B, C) lié à son numéro de série USB
  (`picotool info -a`, ou le fichier `INFO_UF2.TXT` du volume).
- **Montre-moi le tableau carte / série / port / modèle et attends mon accord avant de flasher.**

## Étape 1 : chaîne d'outils

- `rustup target add thumbv6m-none-eabi` (ou la cible RP2350 si besoin).
- Installe `elf2uf2-rs` ou `picotool`. Installe `probe-rs` seulement si une sonde SWD est présente.
- Vérifie que la lib compile pour la cible :
  `cargo build -p oasis-rt --target thumbv6m-none-eabi --lib --no-default-features --features mesh_bloom_mcu --release`

## Étape 2 : firmware de test silicium

Le firmware de nœud n'existe probablement pas encore. Vérifie d'abord, puis écris
un crate binaire minimal `oasis-silicon-test` (`rp2040-hal` ou `embassy-rp`, USB
CDC pour les logs). Il dépend de `oasis-rt` en `no_std` et exécute ces tests sur
la carte, chacun produisant une ligne de log parsable :

| ID | Test | Critère de réussite |
|---|---|---|
| T0 | Boot + ID | numéro de série unique, fréquence horloge, version firmware et hash git |
| T1 | ChaCha20-Poly1305 | vecteur RFC 8439 identique octet par octet |
| T2 | X25519 | vecteur RFC 7748 |
| T3 | Ed25519 sign + verify | vecteur RFC 8032, plus rejet d'une signature altérée |
| T4 | Gate R14 | 1000 fautes au-dessus du seuil, toutes bloquées |
| T5 | Mesh v8/v9/v0A | wrap puis process en local, dédup Bloom, rejet d'une origine usurpée |
| T6 | Timing | cycles mesurés au compteur matériel (SysTick ou TIMER) : R14, wrap v9, sign et verify v0A. K=10 répétitions, médiane ± demi-écart |
| T7 | Mémoire | taille flash et RAM statique (`cargo size` / `arm-none-eabi-size`), plus marge de pile mesurée par stack painting |
| T8 | Persistance `tx_counter` | écriture en flash, coupure USB, reboot, relecture : le compteur ne revient pas à 0 (bloqueur sécurité n°4 de `IOT_READINESS.md`) |

Format de log imposé, une ligne par test :
`OASIS|<board_id>|<test_id>|PASS|FAIL|<valeur>|<unité>|<git_hash>`

## Étape 3 : flasher les 3 cartes

- Une carte à la fois, uniquement sur un volume BOOTSEL identifié à l'étape 0.
- Après chaque flash, vérifie que la carte réapparaît en port série avec le bon numéro de série.

## Étape 4 : exécuter et capturer

- Lis les 3 ports série en parallèle et écris les logs bruts, sans filtrage, dans
  `evidence/silicon/<date>/board_{A,B,C}.log`.
- Lance la série complète 3 fois par carte. T8 exige une vraie coupure
  d'alimentation : demande-moi de débrancher puis rebrancher, et ne la simule pas.
- Calcule le SHA-256 de chaque log et de chaque binaire flashé dans `evidence/silicon/<date>/SHA256SUMS`.

## Étape 5 (optionnelle) : communication entre cartes

Seulement si les 3 cartes sont reliées entre elles (UART croisé : GP0/GP1, GND commun),
fais un test mesh A → B → C en v0A sur ce lien filaire. C relaie, vérifie la
signature et loggue `msg_id`, `hops` et `origin_fp`. Le rapport doit dire
« lien UART filaire », jamais « LoRa ».

## Étape 6 : rapport

Écris `evidence/silicon/<date>/REPORT.md` avec :
- le matériel exact (modèle, série, horloge) et le hash git du firmware ;
- un tableau PASS/FAIL par test et par carte ;
- les timings sur silicium à côté des valeurs x86 et Renode de `IOT_READINESS.md`,
  avec l'écart, sans enjoliver ;
- la liste de ce qui n'est **pas** testé : radio LoRa, énergie, secure element, vol ;
- les commandes exactes pour reproduire.

Mets ensuite à jour la ligne « Real MCU hardware boot » de `CLAUDE.md` avec un
lien vers le rapport, **seulement si T0 à T5 passent sur les 3 cartes**.
Commite sur une nouvelle branche `silicon-test-3pico`, sans pousser sur `main`.

## Quand t'arrêter pour me demander

- Le modèle de carte n'est pas un Pico ou un Pico 2.
- Un volume de flash est ambigu.
- Un test échoue deux fois avec la même cause sans que tu trouves pourquoi.
- Tu devrais modifier la crypto ou la logique R14 de `oasis-rt` pour faire passer un test.
