# OASIS — Plan d'Ingenierie: Validation Hardware Claims 4, 6, 8, 10

**Date**: 12 avril 2026
**Objectif**: Prouver sur hardware reel (Galaxy S24 via Termux) les 4 claims
restants qui sont cables dans le daemon mais jamais valides en session longue.

**Prerequis**: 94/94 tests Rust PASS, test_motor v6 7/7 PASS, daemon v0.3 stable.

---

## STRATEGIE GLOBALE

Un seul binaire `test_claims.rs` qui enchaine les 4 claims dans une session
de ~5 minutes avec capteurs reels. Chaque claim a 3-4 tests specifiques.
Le testeur secoue/deplace le phone pour creer des stimuli naturels.

**Pipeline capteurs → claims:**
```
accelerometre + gyro → momentum → tension field
                                      ↓
              Claim 10: World Model (zones de pression)
                                      ↓
              Claim 4: Branching (choix multi-hypotheses)
                                      ↓
              Claim 6: Morpho (specialisation agents)
                                      ↓
              Claim 8: Dreams (consolidation hors-ligne)
```

---

## CLAIM 4 — TEMPORAL BRANCHING DECISION

### Ce qui est cable (main.rs:265-277)
- 5 branches, toutes les 5 ticks quand R14 safe
- Base force = tension field net, pressure = world model gradient
- Goal fixe a [dim10=0.5, dim27=1.0]
- Modulation par gain emotionnel

### Tests a prouver sur hardware

**T4.1 — Branch Diversity on Real Data**
- Lire 20 samples capteurs, construire le momentum reel
- Brancher 5 hypotheses avec ce momentum comme base_force
- **PASS**: au moins 3 fitness scores distincts (ecart > 0.01)
- **Prouve**: les hypotheses sont reellement differentes, pas du bruit

**T4.2 — Goal Attraction Under Motion**
- Definir un goal a [dim10=1.0]
- Lire capteurs en mouvement (secouer le phone)
- Brancher → verifier que best_direction[10] > 0 (va vers le goal)
- **PASS**: direction[10] > 0 dans au moins 3/5 essais
- **Prouve**: le branching resout vers le goal malgre le bruit capteur

**T4.3 — Pressure Repulsion with World Model**
- Creer une zone repulsive a la position actuelle de l'agent
- Brancher → best_direction doit s'eloigner de la zone
- **PASS**: fitness avec pression > fitness sans pression (le brancher evite)
- **Prouve**: integration branching + world_model fonctionne

**T4.4 — R14 Safety Gate**
- Pousser l'agent en haute entropie (perturbation forte)
- Tenter de brancher → doit etre bloque
- Attendre recovery → branching reprend
- **PASS**: blocked quand entropy > 0.85, allowed quand < 0.85
- **Prouve**: R14 protege les decisions en incertitude

### Inputs capteurs
- Accelerometre: momentum brut → base_force
- World model gradient: calculé depuis position agent + zones
- Entropie agent: pour le gate R14

### Observables
- fitness_scores[] (5 valeurs), best_direction, branches_evaluated
- Deviation pre/post branching (la decision ameliore-t-elle la situation?)

---

## CLAIM 6 — AGENT MORPHOGENESIS

### Ce qui est cable (main.rs:298-305)
- 3 agents enregistres, toutes les 20 ticks
- Threat = fear si > 0.3, unknown = entropy agent 0
- Goal toujours true, healing = 0.0

### Tests a prouver sur hardware

**T6.1 — Differentiation Under Real Threat**
- Phase calme (10 ticks) → tous les agents STEM
- Secouer fort le phone → fear monte → threat > 0.3
- Appeler differentiate() → au moins 1 Sentinel apparait
- **PASS**: count_by_role(Sentinel) >= 1 apres perturbation
- **Prouve**: la morphogenese repond au stress reel

**T6.2 — Role Diversity in Mixed Conditions**
- Alterner calme/agite/calme sur 60 ticks
- threat variable, unknown variable, goal = true
- **PASS**: >= 3 roles distincts presents simultanement
- **Prouve**: diversification emergente sur stimuli naturels

**T6.3 — Redifferentiation on Poor Performance**
- Forcer un agent en Navigator avec performance 0.05
- Attendre 15 ticks (cooldown > 10 ticks)
- Appeler differentiate() → role doit changer
- **PASS**: role different de Navigator apres poor performance
- **Prouve**: reversibilite de la specialisation

**T6.4 — No STEM After Differentiation**
- Appeler differentiate() avec des conditions variees
- **PASS**: count_by_role(Stem) == 0
- **Prouve**: tous les agents se specialisent, pas de zombies

### Inputs capteurs
- Entropies agents (calculees depuis hyper_state)
- Momentum magnitudes (depuis capteurs)
- Fear (depuis emotional state, drivee par pain memories)

### Observables
- Role assignments (Navigator/Sentinel/Worker/Scout/Healer)
- Change count (combien d'agents ont change)
- Commitment levels (flexibilite comportementale)

---

## CLAIM 8 — DREAM CONSOLIDATION

### Ce qui est cable (main.rs:307-315)
- Record toutes les 10 ticks: trajectory=[pos], outcome= +0.5 si calme, -fear si stress
- Dream toutes les 100 ticks SI entropy < 0.3 ET fear < 0.1 (idle)

### Tests a prouver sur hardware

**T8.1 — Experience Recording from Real Sensors**
- Lire 50 ticks de capteurs reels, recorder toutes les 10 ticks
- **PASS**: experience_count() == 5 apres 50 ticks
- **Prouve**: le buffer d'experience se remplit correctement

**T8.2 — Positive Consolidation (reward strengthens)**
- Enregistrer 5 experiences calmes (outcome = +0.5)
- Creer des synapses actives entre agents
- Appeler dream() → synapses doivent etre renforcees
- **PASS**: strengthened > 0 dans le DreamResult
- **Prouve**: les bonnes experiences sont memorisees

**T8.3 — Negative Consolidation + Counterfactual**
- Secouer le phone → fear monte → outcome negatif
- Enregistrer 5 experiences negatives
- Appeler dream() → synapses affaiblies + imagination
- **PASS**: weakened > 0 OU imagined > 0
- **Prouve**: les traumas generent de l'apprentissage contrefactuel

**T8.4 — Idle Trigger Condition**
- Verifier que dream() ne se declenche PAS quand fear > 0.1
- Verifier que dream() SE declenche quand fear < 0.1 ET entropy < 0.3
- **PASS**: dream blocked en stress, allowed en calme
- **Prouve**: consolidation uniquement hors-ligne (pas de reve eveille)

### Inputs capteurs
- Trajectoire agent (positions successives)
- Entropie le long de la trajectoire
- Fear (pour outcome: calme = +0.5, stress = -fear)

### Observables
- experience_count (buffer depth)
- DreamResult: replayed, strengthened, weakened, imagined
- Synapse weight deltas avant/apres dream

---

## CLAIM 10 — NON-EUCLIDEAN WORLD MODEL

### Ce qui est cable (main.rs:233-244)
- Refresh zones toutes les 10 ticks depuis capteurs
- Barometre → zone Entropy (pression atmo = incertitude)
- Motion > 0.1 → zone Repulsive (obstacle en mouvement)
- Goal → zone Attractive (toujours presente)
- Gradient injecte dans le champ de tension (poids 0.5, diffusion 6)

### Tests a prouver sur hardware

**T10.1 — Sensor-Driven Zone Creation**
- Lire barometre + accelerometre
- Creer les zones comme le daemon
- **PASS**: au moins 2 zones actives (entropy baro + attractive goal)
- **Prouve**: les capteurs reels alimentent le modele du monde

**T10.2 — Repulsive Zone on Motion**
- Phase calme → pas de zone repulsive
- Secouer le phone → motion > 0.1 → zone repulsive creee
- **PASS**: repulsion > 0 quand motion, repulsion ~0 quand calme
- **Prouve**: le mouvement cree des zones de danger

**T10.3 — Gradient Navigation Toward Goal**
- Placer agent loin du goal
- Sampler le gradient → doit pointer vers le goal
- Executer navigate() 10 steps
- **PASS**: distance finale < distance initiale
- **Prouve**: descente de gradient fonctionne avec donnees capteur

**T10.4 — Superposition Repulsive + Attractive**
- Creer obstacle repulsif ENTRE agent et goal
- Sampler le gradient → doit contourner (pas foncer dans l'obstacle)
- **PASS**: gradient ne pointe pas directement vers obstacle
- **Prouve**: le champ de pression superpose gere les conflits

### Inputs capteurs
- Barometre: pression → zone entropy
- Accelerometre: magnitude motion → zone repulsive si > 0.1
- Position agent (evoluee depuis hyper_state)

### Observables
- Zone count (actives)
- Gradient vector (direction de navigation)
- Repulsion/attraction magnitudes
- Distance agent-goal avant/apres navigation

---

## ARCHITECTURE DU TEST

### Fichier: `oasis-rt/src/bin/test_claims.rs`

```
Phase 0: Baseline (16 samples, calibration reflexes)
Phase 1: Claim 10 — World Model (T10.1-T10.4)    ~30s
Phase 2: Claim 4  — Branching (T4.1-T4.4)         ~30s
Phase 3: Claim 6  — Morpho (T6.1-T6.4)            ~40s
  → necessite perturbation physique pour fear
Phase 4: Claim 8  — Dreams (T8.1-T8.4)            ~60s
  → necessite calme + stress + calme (3 phases)
SCORECARD: 15 tests, score X/15
```

### Dependances entre phases
- Phase 1 (World Model) produit le gradient pour Phase 2 (Branching)
- Phase 3 (Morpho) utilise la fear de Phase 1-2
- Phase 4 (Dreams) utilise les synapses formees en Phase 1-3
- L'ordre est impose par le pipeline du daemon

### Modules Rust utilises
```rust
use oasis_rt::vec::*;
use oasis_rt::hyper_state::{agent_new, evolve, entropy, is_action_safe};
use oasis_rt::tension::TensionField;
use oasis_rt::synapse::SynapticNetwork;
use oasis_rt::emotion::EmotionalState;
use oasis_rt::reflex::AdaptiveReflex;
use oasis_rt::world_model::{WorldModel, ZoneType};
use oasis_rt::branching::TemporalBrancher;
use oasis_rt::morpho::MorphoEngine;
use oasis_rt::dreams::DreamEngine;
```

### Criteres de reussite
- **15/15**: Tous les claims prouves sur hardware reel
- **12/15**: Acceptable — identifier les 3 echecs et corriger
- **< 12/15**: Investiguer — possible bug dans le wiring daemon

---

## DEPLOY

```bash
# PC: push le test
adb push test_claims.rs //sdcard/test_claims.rs

# Termux:
cp /sdcard/test_claims.rs ~/oasis-rt/src/bin/test_claims.rs
cd ~/oasis-rt
cargo build --bin test_claims
./target/debug/test_claims 2>&1 | tee /sdcard/claims_test.log
```

### Cargo.toml: ajouter
```toml
[[bin]]
name = "test_claims"
path = "src/bin/test_claims.rs"
```

---

## ESTIMATION

| Etape | Effort |
|-------|--------|
| Ecrire test_claims.rs (~350 lignes) | 1 session |
| Deploy + premier run | 10 min |
| Debug echecs + corrections | 1-2 sessions |
| Session longue validation (30 min+) | 1 session |
| Mise a jour claude.md avec resultats | 10 min |

---

## RISQUES

| Risque | Mitigation |
|--------|-----------|
| Barometre indisponible via Termux | Fallback: pression fixe 1013.25 |
| Dream ne trigger pas (fear jamais < 0.1) | Force outcome manuellement |
| Morpho STEM jamais zero (3 agents trop similaires) | Diversifier les momenta |
| Branching toujours meme direction (peu de variance) | Augmenter branches a 8 |
| R10: test_claims.rs > 400 lignes | Split en phases si necessaire |
