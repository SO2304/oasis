# TEST INFAILLIBLE — 11 CLAIMS OASIS

## Principe : chaque test a un critere binaire PASS/FAIL mesurable.
## Un PID seul ou un for-loop echouerait a chaque test.

---

## CLAIM 1 — Tensorial Agent Communication
**Test**: Emettre 2 forces opposees dans le champ. Mesurer l'interference destructive.
**Pass**: net_force < 10% de chaque force individuelle (annulation)
**Fail**: net_force > 50% (pas d'interference, juste addition)
**Plateforme**: Rust unit test (tension.rs already has this: destructive_interference_ratio)
**Mesure**: ratio constructive/destructive dans tf.sample()

## CLAIM 2 — HyperState with Entropy-Gated Actuation  
**Test**: Secouer le telephone fort. Verifier que R14 bloque l'actuation.
**Pass**: r14_safe=0 pendant le shake ET entropy > 0.7
**Fail**: r14_safe=1 malgre entropy > 0.7 (gate ne fonctionne pas)
**Plateforme**: Telephone Android (live session)
**Mesure**: Compter les ticks r14_safe=0 dans le CSV

## CLAIM 3 — Efference Copy
**Test** (Webots): Commander le drone vers un pilier. Predire alt=1.0m. 
Mesurer severity quand alt=1.0 (NOMINAL) vs quand alt=0.5 (ANOMALY) vs quand alt=5.0 (DYSMORPHIA).
**Pass**: Les 3 niveaux apparaissent avec les bonnes valeurs
**Fail**: Tout est DYSMORPHIA (pas de discrimination)
**Test 2** (Phone): Vibrer le telephone. Mesurer l'accelerometre avant/apres. 
**Pass**: Delta accel > 0.5 m/s2 pendant vibration
**Mesure**: Severity counts dans le log

## CLAIM 4 — Temporal Branching
**Test**: Fork 5 timelines avec des conditions differentes. La timeline avec le meilleur fitness gagne.
**Pass**: La timeline choisie a un fitness mesurablment meilleur que les autres
**Fail**: Choix aleatoire ou toujours la meme timeline
**Plateforme**: Rust unit test (branching.rs)
**Mesure**: fitness scores des 5 timelines dans le log

## CLAIM 5 — Emotional Gain Modulation
**Test** (Webots): Approcher un obstacle. Mesurer la vitesse du drone AVEC fear vs SANS fear.
**Pass**: Vitesse avec fear < 70% de vitesse sans fear (fear ralentit)
**Fail**: Vitesse identique (fear ne module rien)
**Test A/B**: Meme waypoint, un run avec OASIS (fear active), un run PID seul.
**Mesure**: vitesse moyenne pendant l'approche

## CLAIM 6 — Agent Morphogenesis
**Test**: 3 agents dans le champ de tension. Apres 1000 ticks, au moins 2 roles differents.
**Pass**: morpho.roles() retourne 2+ roles distincts
**Fail**: Tous le meme role
**Plateforme**: Rust unit test (morpho.rs)
**Mesure**: Distribution des roles dans le log

## CLAIM 7 — Hebbian/STDP
**Test** (Webots): Drone approche un obstacle 5 fois. La synapse obstacle doit:
1. Se former (poids > 0)
2. Se renforcer a chaque approche
3. Moduler le comportement (ralentir plus a chaque fois)
**Pass**: Vitesse a la 5eme approche < vitesse a la 1ere (apprentissage)
**Fail**: Vitesse identique (synapse decorative)
**Mesure**: Poids synaptique + vitesse par approche

## CLAIM 8 — Dream Consolidation
**Test** (Phone): Verifier que les dreams modifient les poids synaptiques.
**Pass**: DREAM log montre s>0 ou w>0 (synapses strengthened/weakened)
**Fail**: s=0 et w=0 sur 100% des dreams (no-op)
**Plateforme**: Telephone (live daemon log)
**Mesure**: Compter s>0 et w>0 dans les DREAM entries

## CLAIM 9 — Reflex Arc
**Test** (Webots): Drone en vol. Simuler un obstacle soudain (<0.12m).
**Pass**: Le drone brake dans le MEME tick (reflex_fired=1, desired_vx=0)
**Fail**: Le drone continue pendant 2+ ticks avant de freiner
**Test 2** (Phone): Secouer le telephone. Reflex doit fire sur le MEME tick que le spike.
**Mesure**: Tick du spike vs tick du reflex fire

## CLAIM 10 — Non-Euclidean World Model
**Test**: Placer 3 zones de pression (repulsive, attractive, entropy). 
Naviguer par gradient descent. L'agent doit eviter la zone repulsive et aller vers l'attractive.
**Pass**: Distance a l'attractive diminue AND distance a la repulsive augmente
**Fail**: Mouvement aleatoire ou pas de navigation
**Plateforme**: Rust unit test (world_model.rs: navigate_avoids_obstacle)
**Mesure**: Trajectoire de l'agent

## CLAIM 11 — Federated Synaptic Resonance
**Test** (Webots swarm): 
1. Drone A decouvre un obstacle, enregistre une fear zone
2. Drone B recoit la fear zone via federation
3. Drone B approche le meme obstacle
**Pass**: Drone B a une Foreign Fear > 0 AVANT d'atteindre l'obstacle.
         Drone B garde plus de marge que Drone A (B_distance > A_distance)
**Fail**: Drone B a FF=0 (federation ne fonctionne pas) 
         ou B_distance <= A_distance (federation n'aide pas)
**Mesure**: FF value + distance minimale au pilier pour A vs B

---

## EXECUTION PLAN

### Phase 1: Rust unit tests (Claims 1, 4, 6, 10)
```
cargo test --lib -- tension::tests
cargo test --lib -- branching::tests  
cargo test --lib -- morpho::tests
cargo test --lib -- world_model::tests
```
Critere: 128/128 pass

### Phase 2: Phone live (Claims 2, 3, 5, 7, 8, 9)
- Secouer le telephone → Claim 2 (R14), Claim 5 (fear), Claim 9 (reflex)
- Vibrer via termux-vibrate → Claim 3 (efference)
- Attendre 10 min → Claim 8 (dreams)
- Analyser CSV → Claim 7 (synapses)

### Phase 3: Webots (Claims 3, 5, 7, 9, 11)
- Obstacle approach A/B test → Claims 3, 5, 7, 9
- Swarm federation test → Claim 11

### Critere global: 11/11 PASS = OASIS PROUVE
