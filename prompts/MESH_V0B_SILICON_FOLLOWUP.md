# Prompt : v0B, suite sur silicium (mode strict, rejeu exact, réserve de compteurs)

À coller dans Claude Code sur le PC où les 3 RP2040 sont branchés, à la racine du
dépôt, après `git fetch` et `git checkout claude/eloquent-ptolemy-oojjn0`. Cette
branche contient `mesh-v0b` plus les corrections de la relecture du 2026-10-06.

---

Lis d'abord `evidence/silicon/2026-10-06/REPORT_MESH_V0B.md` §6 (notes de relecture),
`docs/MESH_V0B_SPEC.md` §5, et `oasis-rt/src/mesh.rs` (`set_allow_legacy`,
`process_v0b`).

## Ce qui a changé depuis ton dernier run

- **Mode strict v0B** : un routeur `new_v0b` refuse désormais v8, v9 et v0A
  (`"legacy envelope rejected by strict v0B router"`), sauf `set_allow_legacy(true)`.
  Le firmware `uart_mesh.rs` démarre en mode legacy (il héberge encore les
  commandes v0A) et gagne `K` (strict) et `k` (legacy).
- `.gitattributes` : `evidence/** -text`. Git ne touche plus aux fins de ligne
  des preuves.
- `uart_mesh.rs` journalise chaque échec CRC.

## Règles

Les règles du prompt `MESH_V0B_SECURITY.md` restent valables. Trois s'ajoutent,
suite aux trois rapports précédents :
1. **Compile depuis un arbre commité et propre** (`git status` vide). Le hash
   dans les logs doit être celui d'un commit poussé.
2. **Écris les logs en LF.** Calcule `SHA256SUMS` sur les fichiers finaux,
   vérifie avec `sha256sum -c`, commite, puis **re-vérifie dans un clone frais**
   du commit (`git clone` du dépôt local dans un dossier temporaire, puis
   `sha256sum -c` dans le dossier de preuves). Si une seule ligne échoue,
   arrête-toi.
3. **Chaque paquet envoyé doit laisser une trace.** Pour chaque balayage, logge
   côté émetteur un numéro de séquence par paquet et côté relais le même numéro,
   pour qu'un paquet perdu soit identifiable et non simplement absent.

## Tâche 1 : réserve de compteurs côté émetteur (code + PC)

Le trou trouvé par T8 : un émetteur qui redémarre repart au compteur 1 et se fait
refuser par les relais.
- Implémente la réserve de compteurs de la spec §5 : l'émetteur écrit en flash un
  **plafond** (`compteur courant + L`, avec `L = 256`) **avant** d'utiliser un
  compteur au-delà de l'ancien plafond. Au démarrage, il reprend **au plafond
  persisté**. Il ne réutilise jamais un compteur.
- Côté PC (`oasis-rt`), expose cette logique dans une API `no_std` testable, avec
  un stockage simulé. Tests : redémarrage à n'importe quel moment, y compris
  pendant l'écriture du plafond (le nouveau plafond est perdu, l'ancien reste
  valide), et aucune réutilisation de compteur sur 10 000 redémarrages aléatoires.
- Ajoute une preuve Kani : le compteur émis après un redémarrage est strictement
  supérieur à tout compteur émis avant.
- Mesure l'usure : nombre d'écritures en flash par message (attendu : 1/256).

## Tâche 2 : silicium

Envoie `K` à A, B et C avant chaque test v0B, et logge la réponse `MODE|strict_v0b=true`.

1. **Balayage d'inversions de bit en mode strict**, 3 × 50, avec numérotation.
   Objectif : 150 paquets tracés, 0 accepté. Toute inversion qui tombe dans le
   magic doit donner `legacy envelope rejected…` ou `bad mesh magic`, jamais un
   passage par l'ancien chemin.
2. **Déclassement sur silicium** : en mode strict, A envoie un message v0A
   (commande `o`) ; B doit le refuser. Puis `k` sur B, et B doit l'accepter : ce
   contrôle négatif prouve que c'est bien le mode strict qui bloque.
3. **Rejeu à l'octet près à travers une coupure** : `Z` sur A (envoie et stocke),
   B accepte et persiste (`P`). Coupe réellement l'alimentation de B (demande-la-
   moi). Au redémarrage, `z` sur A (renvoie les mêmes octets). B doit répondre
   `replay detected` ou `stale counter`.
4. **Réserve de compteurs sur silicium** : A envoie 10 messages, B les accepte ;
   coupe **A** (demande-la-moi) ; au redémarrage, A envoie un nouveau message et B
   doit l'**accepter**. Avant, il était refusé. Logge le compteur repris et le
   plafond lu en flash.

## Livrables

- Logs bruts dans `evidence/silicon/<date>/`, `SHA256SUMS` re-vérifié après commit.
- Un `REPORT.md` court : tableau test → résultat attendu → résultat obtenu → log.
- Mets à jour `docs/SECURITY_COMPARISON.md` et `docs/MESH_V0B_SPEC.md` §5 (réserve
  de compteurs : fait ou non, avec ses limites).
- Commite sur la branche courante et pousse.

## Quand t'arrêter

- Si un test échoue deux fois pour la même raison.
- Si une somme de contrôle échoue après le commit.
- Avant chaque coupure de courant.
