# OASIS — Skills Operationnels

Ce fichier decrit les VRAIES competences necessaires pour developper OASIS.
Pas de skills theoriques ou entreprise — uniquement ce qu'on utilise chaque jour.

---

## 1. Kernel Rust (priorite: critique)

**Quand**: Chaque nouveau module, chaque fix, chaque test.

- Ecrire du Rust no_std-compatible, zero allocation quand possible
- Arrays fixes `[f64; DIM]` au lieu de Vec pour le hot path
- `#[inline(always)]` sur les fonctions vectorielles critiques
- Compiler pour aarch64 (ARM Android) via Termux
- Debug mode pour iteration rapide, release avec LTO pour prod
- Gerer les overflows (`saturating_sub`, `checked_add`)
- Serde pour deserialiser la telemetrie capteurs JSON

**Contraintes**:
- Latence kernel < 1ms par tick
- Jitter < 5%
- Zero panic en production (tous les unwrap doivent etre prouves safe)
- Fear saturee a 5.0 (pas d'explosion emotionnelle)
- Capteurs invalides → valeur safe (lux > 100000 → 0, pression hors range → 1013.25)
- Eligibility decay meme en silence (pas de credit zombie)

---

## 2. Deploy Android/Termux (priorite: critique)

**Quand**: Chaque test, chaque session de validation.

```bash
# Push fichier vers le telephone
adb push fichier.rs //sdcard/fichier.rs

# Dans Termux: copier vers le projet
cp /sdcard/fichier.rs ~/oasis-rt/src/bin/

# Build debug (rapide, ~2s pour incremental)
cargo build --bin nom_du_test

# Build release (lent avec LTO, eviter sauf prod)
cargo build -r --bin oasis-rt

# Lancer le daemon en arriere-plan
nohup ./target/debug/oasis-rt > ~/oasis-log.txt 2>&1 &
termux-wake-lock

# Verifier que ca tourne
ps aux | grep oasis
tail -5 ~/oasis-log.txt
```

**Pieges connus**:
- `input text` via ADB ne passe pas `>` et `&` correctement → utiliser des scripts .sh pushes sur sdcard
- `lto = true` en release prend 15-20min sur ARM → build debug pour les tests
- Le telephone deconnecte USB en veille → `termux-wake-lock` AVANT de lacher le cable
- `[[bin]]` doit etre declare dans Cargo.toml pour chaque binaire de test
- Les ticks du SynapticNetwork et du FederatedMesh sont des compteurs DIFFERENTS → `saturating_sub`

---

## 3. Tests Impitoyables (priorite: critique)

**Quand**: Chaque mecanisme doit etre prouve avant d'etre considere valide.

**Structure d'un test OASIS**:
1. Lire les capteurs reels (accelerometre, gyroscope)
2. Convertir en momenta vectoriels N-dimensionnels
3. Executer le module a tester avec les donnees reelles
4. Verifier chaque propriete mathematique (monotonie, bornes, directions)
5. Afficher un scorecard colore avec PASS/FAIL

**Tests existants**:
- `cargo test --lib` — 54 tests Rust (vec, hyper_state, tension, synapse, emotion, reflex, federation, morpho)
- `test_hebbian.rs` — 7 tests telephone: formation, non-formation, potentiation, depression, pruning, STDP direction, reward credit
- `test_federation.rs` — 5 tests telephone: harvest, resonance sympathique, rejection, trust gating, avantage collectif
- `pc_bridge.rs` — relais temps reel Phone→PC via ADB
- `pc_replay.rs` — replay du log telephone dans le kernel PC (preuve cross-device learning)
- `npx vitest run` — 377 tests TypeScript (specification kernel)

**Regle**: Score < 100% = on corrige le code OU le test, jamais on ignore.
**Regle**: Chaque bug corrige doit avoir un test qui le couvre (regression guard).

---

## 4. Capteurs Reels (priorite: haute)

**Quand**: Chaque test, chaque session daemon.

**Capteurs disponibles** (Samsung Galaxy S24):
- `LSM6DSVTR Accelerometer` — 3 axes, m/s², ~100Hz
- `LSM6DSVTR Gyroscope` — 3 axes, rad/s, ~100Hz
- `LPS22DF Barometer` — pression atmospherique, hPa
- `TMD3702V Light Sensor` — luminosite, lux

**Lecture via Termux**:
```bash
termux-sensor -s "LSM6DSVTR Accelerometer,LSM6DSVTR Gyroscope" -n 1
```

**Conversion en vecteur latent**:
```rust
fn accel_to_momentum(accel: [f64;3], gyro: [f64;3], scale: f64) -> V {
    let mut m = vz();
    m[10] = accel[0] * scale;
    m[11] = accel[1] * scale;
    m[12] = (accel[2] - 9.81) * scale; // retirer la gravite
    m[13] = gyro[0] * scale * 3.0;
    m[14] = gyro[1] * scale * 3.0;
    m[15] = gyro[2] * scale * 3.0;
    m
}
```

---

## 5. Analyse de Donnees Long-Run (priorite: haute)

**Quand**: Apres chaque session daemon (4h+).

**Log CSV du daemon** (~/oasis-log.txt ou CSV separe):
- Colonnes: tick, epoch_ms, ax, ay, az, gx, gy, gz, pressure, light, entropy, collapsed_state, fear, curiosity, satisfaction, frustration, urgency, dominant_emotion, synapses, r14_safe, reflex_fired, kernel_us

**Analyses a faire**:
- Stabilite de l'entropie sur la duree (convergence ou divergence ?)
- Evolution du nombre de synapses (croissance, plateau, pruning ?)
- Distribution des emotions (dominance, transitions)
- Frequence des reflexes (adaptation ou repetition ?)
- Latence kernel (moyenne, P99, jitter)
- Correlation capteurs ↔ etats internes

---

## 6. Architecture Bio-Inspiree (priorite: conception)

**Quand**: Conception de nouveaux modules.

**Principes**:
- Le cerveau resout le probleme avant nous → chercher l'analogie biologique
- Pas de messages discrets → champs de force continus
- Pas de planification centralisee → emergence par gradient
- Pas de base de donnees → la topologie EST la memoire
- Pas d'optimisation prematuree → laisser le systeme apprendre

**Analogies actives**:
| Biologie | OASIS |
|----------|-------|
| Synapse | Connexion ponderee entre agents (Hebbian) |
| STDP | Directionnalite causale de l'apprentissage |
| Copie efferente | Prediction proprioceptive pre-action |
| Arc reflexe | Bypass deliberation pour urgences |
| Emotions | Multiplicateurs de gain sur le comportement |
| Reve | Consolidation hors-ligne par replay |
| Neurone miroir | Resonance federee (Mechanism 11) |
| Champ recepteur | Zone d'influence dans le champ de tension |

---

## 7. Securite Physique (priorite: critique)

**Quand**: Tout agent avec actuation physique.

- R14: Entropie > seuil → gel de l'actuation (pas de mouvement dans l'incertitude)
- R15: Perte de capteur → entropie max immediate
- R16: Deviation intention/realite → douleur + reinit posture
- Kill switch: toujours accessible, jamais bypassable
- Reflex arc: reaction < 100µs, AVANT toute deliberation

---

## NON-SKILLS (ce qu'on ne fait PAS)

- Multi-tenant enterprise (OASIS n'est pas un SaaS)
- API REST / GraphQL (les agents communiquent par tension)
- Base de donnees relationnelle (la topologie est la memoire)
- CI/CD cloud (on deploy sur du hardware physique)
- Frontend web (pas d'interface utilisateur classique)
- Orchestration style microservices (pas de service mesh classique)
- Event sourcing enterprise (le log CSV suffit)
- Auth0 / OAuth / JWT (les agents s'authentifient par signature cryptographique)

---

## Defauts Connus (honnetete)

- Daemon meurt apres ~1h sur Android (OOM killer ou restriction batterie) → watchdog relance
- 1 seule synapse se forme en prod (3 agents trop similaires) → besoin de plus d'agents diversifies
- Federation Phone→PC unidirectionnelle (PC ne renvoie rien au phone)
- Mechanisms 3, 4, 8, 10 jamais testes sur hardware reel
- Le "377 tests TypeScript" teste la specification, pas le runtime Rust qui tourne en prod
- Le replay PC est offline (pas temps reel pendant le trauma)
