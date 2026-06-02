# OASIS — Cartographie

État au 2026-04-18. Une ligne par dossier/artefact top-level : nom, rôle, statut, dépendances si actif.

## Code source

- **`kernel/`** — Spécification TypeScript du système nerveux OASIS (~16 860 lignes, 20+ sous-modules : auth, bridge, bridge-os, emotion, ghost, hal, ipc, morpho, neuro, optim, perception, persistence, physics, scheduler, shell, sim). **Actif**. Dépend de : `vitest` (tests), `crypto` Node stdlib, `package.json` deps. Le sous-module `bridge-os/inter-kernel.ts` vient d'être durci (Ed25519 sovereign, rate limit, ring buffer — 16/16 tests passent).

- **`oasis-rt/`** — Runtime Rust production (22 modules lib + 18 binaires, 128 tests unit). C'est le vrai cerveau déployé : HyperState, WorldModel, EmotionalState, SynapticNetwork, FederatedMesh, Reflex, Vitality, etc. `src/bin/drone_bridge.rs` sert de cerveau pour les drones Webots. **Actif**. Dépend de : `serde`, `serde_json`, Cargo (edition 2021). Build via `cargo build --release --bin drone_bridge`.

- **`webots/`** — Simulation 3 drones usine (world `oasis_factory.wbt`, Python thin-client `oasis_factory.py` qui pipe sensors↔motors au drone_bridge.exe). Contient aussi `ab_results/`, `ablation_results/`, `transfer_results/` de toutes les expériences A/B. **Actif**. Dépend de : Webots R2023b+, `oasis-rt/target/release/drone_bridge.exe`, Python (controller runtime Webots).

- **`crazyflie-bridge/`** — Prototype wrapper cflib pour porter OASIS sur Crazyflie 2.1 (SITL ou hardware). **Expérimental** (non testé en vol). Dépend de : `cflib` Python, `oasis-rt/target/release/drone_bridge.exe`. Nécessite achat hardware (~$385) pour validation réelle.

- **`phone_brain/`** — État persistant extrait du daemon phone (OASIS-RT v0.6, session 28h39m, 1M ticks) : `oasis-pain.bin` (128 pain memories), `oasis-memory.bin` (1 federation digest). **Actif** (input snapshot pour tests transfer). Dépend de : ADB pull depuis `/sdcard/oasis-*.bin`.

## Artefacts de test / démo

- **`demo-android.ts`, `demo-phone.ts`, `test-train.ts`** — Entrées standalone pour dist bundling. **Expérimental** (démos isolées). Dépend de : `kernel/`.

- **`browser-entry.ts`** — Build entry pour exposer OASIS en bundle navigateur. **Expérimental / peu utilisé**.

- **`dist/`** — Output TypeScript compilé (`oasis-kernel.mjs`, `oasis-browser.mjs`, `oasis-adb.mjs`). **Actif** (généré par `npm run build`). Dépend de : `kernel/`, `tsconfig.json`, scripts build.

## Configuration / meta

- **`package.json`, `tsconfig.json`, `vitest.config.ts`, `package-lock.json`** — Config Node/TS. **Actif**.

- **`Dockerfile`** (racine) — Ancien Dockerfile TypeScript OASIS kernel (`node:20-alpine`). **Abandonné au profit de** `oasis-rt/Dockerfile` (Rust multi-stage).

- **`.github/workflows/ci.yml`** — CI matrix 3 OS × stable+beta, clippy, cargo-audit, Docker build. **Actif**. Dépend de : `oasis-rt/Cargo.toml`, `oasis-rt/Dockerfile`.

- **`node_modules/`** — Dépendances Node (vitest, typescript, etc.). **Actif**, auto-géré.

## Docs

- **`README.md`** — README public avec badge CI, status des 11 mécanismes, architecture diagram ASCII, quickstart Docker/cargo/Webots, roadmap. **Actif**.
- **`CLAUDE.md`** / `claude.md` — Instructions projet, status 11 mécanismes bio-inspirés, règles R1-R20. **Actif**, source de vérité interne.
- **`CONTRIBUTING.md`** — Règles contribution (R10 < 400 lignes, strict typing). **Actif**.
- **`SECURITY.md`** — Threat model, R9/R14/R15, limites connues (pool_push_test, federation trust). **Actif**.
- **`LICENSE`** — MIT. **Actif**.
- **`RAPPORT-OASIS.md`** — Rapport interne. **Statut incertain**, probablement obsolète.
- **`PLAN-CLAIMS-4-6-8-10.md`, `PLAN-TEST-11-CLAIMS.md`** — Anciens plans de test. **Abandonnés** (remplacés par les sessions phone + Webots réelles).
- **`skills.md`** — Non lu, statut incertain.

## Dossiers auto-générés / logs

- **`webots/factory_*.log`, `webots/factory_shared/`** — Logs runtime + federation binaires par run. **Éphémère** (écrasé à chaque run).
- **`webots/ab_results/`, `ablation_results/`, `transfer_results/`** — Archives batches expérimentaux. **Actif** (données historiques utilisables).
- **`oasis-rt/target/`** — Cargo build output. **Auto-généré**.

## Résumé de santé

| Zone | Lignes / taille | Tests | État |
|------|-----------------|-------|------|
| `kernel/` TypeScript | ~16 860L | 377 (vitest) | Actif, certains tests externes à inter-kernel.ts ont des erreurs TS pré-existantes |
| `oasis-rt/` Rust | ~5 500L | 128 unit | Actif, drone_bridge.exe production-ready |
| `webots/` sim | controller 180L + runners bash | batches archivés | Actif, 3 drones fonctionnels avec bad_spots |
| `crazyflie-bridge/` | 180L Python | 0 | Expérimental, hardware requis |
| `phone_brain/` | 28 KB data | — | Actif comme snapshot |

**Chemin minimum pour faire tourner OASIS**: `cargo build --release --bin drone_bridge` dans `oasis-rt/`, puis `webots --mode=fast webots/worlds/oasis_factory.wbt`. Le reste (`kernel/` TS, `dist/`, demos) est optionnel pour la couche Rust production.
