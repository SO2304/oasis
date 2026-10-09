# Ce PC comme automate de test — HMI → agent → passerelle → appareil, en vrais processus

**2026-10-09.** Trois exécutions, **32 cas sur 32** chacune. Reproduire :

```bash
cargo build --release -p oasis-rt -p oasis-test-plc
bash tools/pilot_campaign.sh /tmp/camp
```

| | |
|---|---|
| Résultat | **32/32**, trois fois ([`run1.log`](run1.log), [`run2.log`](run2.log), [`run3.log`](run3.log)) |
| Écritures appliquées par l'appareil | 208 par exécution, comptées **par l'appareil** |
| Latence bout en bout | **1 017 / 1 159 / 1 248 µs** médiane, ±8–15 % (K=10 × 20, une socket tenue) |
| Journal | **intact, exit 0**, et un refus réécrit en acceptation → **exit 1** |

## 1. Pourquoi ce n'est pas le test d'intégration qui existait déjà

`oasis-rt/tests/mbtcp_pilot_sockets.rs` passe par de vraies sockets, mais tout vit dans un
seul processus de test : il prouve la **bibliothèque**. Cette campagne lance les
**binaires livrés**, donc elle exerce ce qu'aucun test en processus ne touche — fichiers de
configuration, graines lues sur disque, écouteur, fil par connexion, **fichier de séquence
persisté**, **compteur d'émission persisté**, **journal sur disque**, et un **redémarrage**
de chaque processus.

```text
hmi_client.py  --Modbus TCP-->  oasis_mbtcp_agent  --v0B-->  oasis_mbtcp_gateway
                                                                    |
                                                          Modbus TCP v
                                                            oasis_test_plc  (rmodbus)
```

**Aucune des deux extrémités n'est du code OASIS, et c'est vérifiable** :
`oasis-test-plc/Cargo.toml` dépend de `rmodbus` et **pas** de `oasis-rt` ; le client est du
Python sans aucun import. Quand l'appareil applique une trame, OASIS ne se donne pas raison
à lui-même — la même propriété que la carte A sous `rmodbus` en phase 1.4.

## 2. Les 32 cas

| # | Ce qui est vérifié | Vérité de terrain |
|---|---|---|
| C1 | une écriture légitime atteint l'appareil, qui **garde la valeur** | compteur et registre imprimés **par l'appareil** |
| C2 | registre hors carte → 0x02, **l'appareil n'est pas interrogé** | compteur inchangé |
| C3 | valeur hors plage → 0x03, idem | compteur inchangé |
| C4 | une **lecture** traverse la passerelle et rend ce que l'appareil détient | `values=500` |
| C5 | lecture hors carte → 0x02 sans interroger l'appareil | compteur inchangé |
| C6 | du Modbus brut **sur le port de la passerelle** n'atteint pas l'appareil | `NOANSWER`, compteur inchangé |
| C7 | la séquence de l'agent **survit à un redémarrage** et ne se réutilise pas | fichier 64 → 128, écriture acceptée après |
| C8 | le journal est **sur disque** et vérifie avec le vérificateur indépendant | `exit 0`, acceptations **et** refus nommés |
| C9 | un **refus réécrit en acceptation** est détecté | `exit 1` |
| C10 | un redémarrage de passerelle change le `boot_id` et ouvre une chaîne neuve, intacte | deux `JRN_BOOT` différents, deux `exit 0` |
| C11 | la latence à travers les **vrais binaires** | 1 017–1 248 µs |
| C12 | un ordre adressé à **un autre `gateway_id`** est refusé | 0x0A, journal `Reject(NotAuthorized)`, compteur inchangé |
| C13 | une origine **révoquée** est refusée — **au niveau mesh, au-dessus du portail** | `MESH_DROP why="origin revoked"`, journal `Reject(NotVerified)` |
| C14 | le journal **attribue** la décision à une origine | `origin=aa00` sur une décision vérifiée |

## 3. Trois défauts trouvés en faisant tourner les vrais programmes

Aucun n'était visible depuis un test en processus. C'est la raison d'exister de cette
campagne.

### 3.1 Le journal de la passerelle n'était pas écrit du tout

`handle_frame` appelait `Journal::append`, **jetait les octets rendus**, et gardait la
chaîne en RAM : le journal mourait avec le processus, pendant que `CLAUDE.md` écrivait
« appends **every** decision to the journal, refusals included ». C'était vrai et sans
valeur — l'annexe III 1.1.9 alinéa 5 demande de **recueillir** la preuve, et un
enregistrement qui disparaît à la sortie ne recueille rien. Le firmware RP2040 persistait
via `jstore` depuis le début ; la passerelle hôte, jamais.

Corrigé : `JournalSink`, deux fichiers, **entrée d'abord puis tête** comme `jstore` sur le
MCU, au format que `oasis_journal_verify` lit déjà. Et toutes les écritures passent par
**une** fonction, pour qu'une branche future ne puisse pas enregistrer en RAM seule en
oubliant une ligne — ce à quoi ressemblait exactement le défaut d'origine.

### 3.2 L'agent se verrouillait lui-même hors de sa passerelle après un redémarrage

C7 a échoué d'abord : toute écriture après un redémarrage de l'agent revenait en exception
0x0B. La séquence d'ordre était persistée (`SeqStore`), mais le **compteur d'émission v0B**
ne l'était pas : un agent redémarré repartait à 1 et la fenêtre anti-rejeu de la passerelle
refusait ses enveloppes comme périmées — **correctement**. `tx_lease` résout exactement ce
problème sur le MCU et a été prouvé sur silicium à travers une coupure réelle ; l'agent hôte
ne l'avait simplement jamais eu, et aucun test en processus ne redémarre un processus.

Corrigé : `TxCounterStore`, même bail que la séquence (256 compteurs par écriture), même
garantie — un plantage en perd jusqu'à 256 et n'en **réutilise** jamais un.

### 3.3 Une écriture coûtait 13,2 ms, dont 12 de réouverture de fichiers

La première version de `JournalSink` rouvrait les deux fichiers à chaque décision :
13 203 µs par écriture autorisée contre ~460 µs en processus. En gardant les deux
descripteurs ouverts et en réécrivant la tête sur place : **1 230 µs**, soit **11× moins**,
et l'ordre de durabilité est identique. `TCP_NODELAY` a aussi été posé sur la socket face à
l'HMI — la passerelle et le lien vers l'appareil l'avaient, celle-là non — mais ce n'était
**pas** la cause : la mesure n'a pas bougé, et c'est dit ici plutôt que présenté comme un
gain.

Reste donc ~770 µs au-dessus du chiffre en processus : les deux `flush` du journal et les
frontières de processus. **La preuve coûte de la latence**, et le chiffre honnête est 1,2 ms
par écriture autorisée, journal persisté compris.

## 3bis. Le portail tournait sur trois entrées constantes

Trouvé en relisant ce que la campagne exerçait réellement. Le portail est bien celui de la
partie F, **inchangé** — mais il ne vaut que ce qu'on lui donne, et la voie TCP lui donnait :

```rust
OrderContext { v0b_ok: true, authorized: true, revoked: false, ..., r14_safe: true }
```

là où le firmware lui donne `registry.allows(origin, ACTUATE)`, `router.is_revoked(origin)`
et `efs.r14_safe_now()`. Conséquences, et ce qui en a été fait :

- **`gateway_id` n'était jamais comparé.** Le firmware plie cette comparaison dans
  `authorized` ; la voie TCP la codait à `true`, donc la passerelle 1 exécutait un ordre
  adressé à la passerelle 7. C'est le défaut réel des trois, et **C12** le montre : mêmes
  clés, `gateway_id = 7`, refus 0x0A, `Reject(NotAuthorized)` au journal, appareil intact.
- **La révocation ne faisait rien** sur cette voie. Elle en fait maintenant — mais **C13**
  montre que le vrai garde-fou est **une couche au-dessus** : v0B jette une origine révoquée
  au premier saut, avant même de vérifier sa signature, donc l'entrée `revoked` du portail
  est de la **défense en profondeur** et ne se déclenche pas ici. C'est dit comme ça plutôt
  que de laisser croire que le portail a arrêté quelque chose. ⚠️ La liste est de
  **configuration** : un `ORV1` signé sur ce lien n'est pas traité, et c'est le manque.
- **Le journal écrivait `origin=0000`** à chaque entrée : il disait qu'une décision avait eu
  lieu, pas **qui** l'avait causée, alors que l'alinéa 5 parle d'une intervention
  légitime *ou illégitime* — une affirmation d'attribution. **C14** : `origin=aa00` sur une
  décision vérifiée. Et `0000` est **conservé** sur une enveloppe rejetée, délibérément :
  son champ d'origine est contrôlé par l'attaquant jusqu'à vérification de la signature, et
  inscrire une prétention non vérifiée dans la preuve est pire que de n'en inscrire aucune.

Deux des quatre constantes restent, en commentaires et non en code : `v0b_ok` est vraie par
construction (la couche mesh a vérifié l'enveloppe au-dessus) et `r14_safe` n'a **aucun
capteur à lire** sur un hôte — une passerelle colocée avec de la mesure devra l'alimenter.

⚠️ Reste plus faible que le firmware : la config TCP porte des **clés**, pas des
**permissions**, donc une clé qui y figure peut commander tout ce que la carte de registres
autorise. Le modèle d'enrôlement (`ACTUATE`) n'est pas sur cette voie.

## 4. Six défauts de la campagne elle-même

Écrits ici parce qu'un harnais qui mentait une fois mentira encore.

1. **Cinq cas « PASS » alors que rien ne tournait.** La première exécution lançait les
   trois processus, qui mouraient tous (chemins MSYS dans les configs, illisibles pour des
   binaires Windows natifs), et les onze cas tournaient contre du vide. Trois sont passés
   en comparant `"0\n0"` à `"0\n0"` — `grep -c` rend 1 quand il ne trouve rien, donc le
   repli `|| echo 0` s'ajoutait à son propre `0`. Corrigé par un compteur `awk` qui rend
   toujours un entier, **et** par une barrière de disponibilité qui **abandonne** si l'un
   des trois ports n'accepte pas de connexion. Une connexion refusée ressemble à un ordre
   refusé pour qui ne vérifie que « est-ce que ça a échoué ».
2. **Un contrôle négatif qui ne mutait rien.** C9 cassait un octet de décision à l'entrée 0,
   qui était un `Act` dont l'octet valait déjà `00` : il écrivait `00` sur `00`, ne changeait
   rien, et concluait « falsification NON détectée » à propos d'une mutation inexistante. Il
   cherche maintenant une entrée dont l'octet n'est pas `00`, et **abandonne** s'il n'en
   trouve aucune.
3. **Une latence de 151 ms.** C11 lançait un processus Python par écriture et appelait le
   résultat une latence. Mesuré dans un seul processus sur une socket tenue : 1,2 ms.
4. **C12 passait sur le mauvais refus.** Le second agent avait ses propres fichiers de
   séquence et de compteur — ils sont nommés d'après le **fichier de config**, alors qu'ils
   appartiennent à l'**identité** — donc son compteur v0B repartait à 1, la fenêtre de la
   passerelle refusait l'enveloppe comme périmée, et le cas était vert sur un 0x0B sans que
   l'ordre ait jamais atteint le portail. Corrigé en partageant les deux fichiers ; la note
   de conception est que ces chemins devraient dériver de l'empreinte, pas du nom de
   fichier.
5. **Un `sed` coupé en deux.** Un patch avait écrit un vrai retour à la ligne dans
   l'expression `sed` qui ajoutait la ligne `revoked`, ce qui cassait la commande en deux et
   produisait une config que la passerelle refusait (`missing network_id`). Remplaçé par un
   `cat` + `echo`.
6. **C14 passait sur une chaîne vide.** Il ne testait que l'égalité à `origin=0000`, donc la
   valeur vide laissée par un C13 en échec passait. Un cas qui passe quand son entrée manque
   est pire que pas de cas.

## 5. Ce que cette campagne ne prouve pas

- **Ce n'est pas un automate.** `oasis_test_plc` est un simulateur d'appareil : pas de cycle
  de scrutation, pas de logique à relais, pas d'E/S, pas de temps réel, et il répond
  instantanément là où un automate prend des millisecondes. Il rend **le protocole et le
  chemin d'autorisation** réels ; il ne rend pas la machine réelle. La latence mesurée à
  travers lui est un **plancher**.
- **Une seule machine, en loopback.** Pas de réseau, pas de commutateur, pas de radio, pas
  de RS-485. Les trois processus partagent un ordonnanceur et un cache.
- **L'injection Modbus brute ne laisse aucune entrée de journal.** Ses quatre premiers
  octets ne sont pas un préfixe de longueur valide, donc `read_frame` la refuse avant
  qu'elle devienne une décision. Rien n'atteint l'appareil — c'est la propriété de sûreté —
  mais il n'y a **pas de trace de la tentative**. Ne pas la journaliser est délibéré (une
  inondation au niveau du cadrage évincerait l'historique des commandes de l'anneau, la même
  raison que pour les lectures), donc c'est une **limite connue de la piste d'audit**, pas
  un oubli. C'est écrit dans le log de chaque exécution.
- **Les ordres sont servis un à la fois** : le verrou d'état couvre l'aller-retour vers
  l'appareil. Correct pour un appareil, plafond de débit pour un parc.
- **Aucun ordre d'autorité** ne passe par ce chemin, donc le journal des **changements**
  (1.1.9 al. 5, 2ᵈ et 3ᵉ déclencheurs) n'est pas exercé ici : il l'est par ses tests et ses
  6 harnais Kani, et son câblage firmware n'a pas été flashé.
