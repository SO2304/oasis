# Prompt : optimisation technique maximale d'OASIS (après l'audit du 2026-10-09)

À coller dans Claude Code à la racine du dépôt OASIS, sur le PC où les 3 RP2040 et les
3 RP2350 sont branchés, avec un second PC relié par câble Ethernet. Base : `main`
(`6865f43` ou plus récent). Lis d'abord, dans cet ordre : `CLAUDE.md`,
`partners/POSITIONING_GAPS.md` §G, `docs/REPORT_POSITIONING_ALIGNMENT.md` §4–§7,
`docs/compliance/README.md`, `evidence/pilot/2026-10-09/README.md`,
`prompts/OASIS_KEY_PROTECTION_RP2350.md`, `tools/check_claims.sh`.

---

## 0. Ce que l'audit a établi, et que ce prompt doit respecter

- **Le potentiel d'OASIS est étroit et daté** : autorisation **par ordre** (émetteur,
  fraîcheur, registre, plage), vérifiée à chaque relais, arrêt jamais bloqué, journal des
  refus, sur automates existants, avec preuves formelles. Aucun produit public ne coche ces
  six cases ; un Internet-Draft IETF d'août 2026 (`draft-morrison-ot-command-authority`)
  décrit le même design. **Tout ce que tu fais doit renforcer ces six cases, ou lever un
  manque qui empêche de les vendre.** Rien d'autre.
- **Ordre de valeur** : 1) ce qui permet un pilote payant devant une machine Modbus TCP ;
  2) ce qui lève la première objection d'un acheteur (clé lisible) ; 3) ce qui prouve la
  face embarquée (radio, autres puces) ; 4) le reste.
- **Trois phrases restent interdites** dans tout document, commit ou rapport :
  « résistant au déni de service », « journal infalsifiable », « prouvé sur le terrain ».
  Les formes exactes autorisées sont dans `docs/REPORT_POSITIONING_ALIGNMENT.md` §6.
- **Le segment du dépôt est S1** (machines mobiles autonomes) ; **le plan de vente est la
  passerelle** (S2). Les deux coexistent : la passerelle finance, S1 est le cap. Écris-le
  ainsi partout où les documents se contredisent.

## 1. Compétences à mobiliser (change de casquette explicitement, à chaque phase)

| Casquette | Quand | Ce qu'elle exige |
|---|---|---|
| **Ingénieur Rust embarqué `no_std`** | tout code dans `oasis-rt`, firmware, bootloader | pas d'allocation cachée, tailles flash/RAM mesurées avec la commande qui les produit |
| **Relecteur cryptographique** | tout ce qui touche clés, signatures, compteurs, journal | primitives standard uniquement (RFC 8032, 7748, 8439, FIPS 204) ; domaine de séparation ; pas de crypto maison |
| **Vérificateur formel (Kani)** | chaque règle pure modifiée ou ajoutée | harnais + **contre-épreuve qui échoue** ; un « FAILED » n'est une réfutation que s'il nomme une propriété, pas une *unwinding assertion* |
| **Fuzzeur** | chaque parseur nouveau ou modifié | cible `cargo-fuzz`, heures et exécutions rapportées séparément des campagnes précédentes |
| **Ingénieur OT / Modbus** | passerelle, agent, automate | adressage 0 vs 40001, FC03/06/16, exceptions 0x02/0x03/0x0A/0x0B, un seul chemin vers l'automate |
| **Responsable conformité produit** | tout document de `docs/compliance/` et `partners/` | texte vérifié à la source ou marqué « non vérifié » ; jamais « conforme », jamais « certifié » |
| **Ingénieur CI** | `.github/workflows`, `tools/` | tout chiffre annoncé est produit par une commande ; `check_claims.sh --full` vert après chaque commit |
| **Auditeur de soi-même** | fin de chaque phase | la section « ce que ma vérification a trouvé » du rapport, défauts compris |

## 2. Règles non négociables

1. **La règle de décision ne change pas** (`actuation_decision`, `stop_decision`,
   `gateway_decision`). Elle est prouvée et sur silicium. Tout changement de ses conditions
   est refusé, sauf un défaut démontré par un test ou un harnais, et alors documenté.
2. Un seul site d'écriture vers un automate, qui ne reçoit qu'une trame issue d'un `Act`.
3. **Avant et après chaque commit** : `cargo test --workspace --release`, `cargo clippy`
   sans nouvel avertissement, `cargo fmt --all -- --check`, le build MCU de `CLAUDE.md`,
   `oasis-rt/kani_shards.sh --check`, `bash tools/check_claims.sh --full`.
4. Chaque harnais Kani nouveau est **vérifié**, avec une contre-épreuve dont la mutation
   est **vérifiée appliquée** (le script aborte sinon). Les journaux `.log` des preuves sont
   **ajoutés en force** (`git add -f`), car `*.log` est ignoré : l'audit a trouvé un dossier
   de preuves qui citait quatre journaux jamais commités.
5. **Chiffres** : K=10, médiane ± demi-écart, jamais un tir unique. L'empreinte de l'arbre
   (`tree=`) dans un journal de campagne doit être **le commit testé** : si le code n'est
   pas commité, commite d'abord, puis mesure.
6. **Honnêteté** : chaque limite est écrite dans la spec, `CLAUDE.md` et la section G.
   Ne retire jamais un ⚠️ sans la preuve qui le lève.
7. **Rust uniquement** dans le code et les outils. Exceptions tolérées et déclarées :
   `tools/hmi_client.py` (l'IHM ne doit pas être du code OASIS) et les scripts de
   contre-épreuve existants. Pas de nouveau Python.
8. **OTP irréversible** : aucune écriture d'OTP sur un RP2350 sans la séquence écrite, une
   simulation, et l'accord explicite de l'utilisateur, carte par carte. Carte **E** seule
   pour les essais ; F et G restent vierges jusqu'à preuve complète sur E.
9. Commits sur une branche de travail, jamais sur `main` directement ; aucune fusion sans
   demande explicite ; **aucun identifiant de modèle** dans le dépôt.
10. Quand une phase révèle que la suivante repose sur une hypothèse fausse, **arrête-toi
    et dis-le**, au lieu de continuer.

## 3. Phase 0 : hygiène documentaire (une journée, avant tout code)

L'audit a trouvé ces incohérences. Corrige-les, chacune avec la commande ou le fichier qui
fait foi :

1. `docs/compliance/MACHINERY_REGULATION_2023_1230.md` : la ligne **al. 3** dit « aucune
   liste des logiciels essentiels » alors que `SOFTWARE_INVENTORY.md` existe ; la partie
   **1.2.1** cite « 152 harnais, 130 vérifiés » et marque C2 « ouvert » alors que la partie G
   est sur silicium.
2. `partners/POSITIONING_GAPS.md` §G : **B4** cite « 23 cas sur 23 » (la campagne est à
   42/42) ; **C13** « fermé avec preuve » contredit `REPORT_POSITIONING_ALIGNMENT.md` §5
   « aucun travail fait » ; les totaux du rapport (9 / 14) ne correspondent pas à G (10 / 13).
3. `partners/DIFFERENTIATORS.md` : l'identifiant **M6** est utilisé deux fois.
4. `evidence/pilot/2026-10-09/README.md` : la latence annoncée (1 017 / 1 159 / 1 248 µs)
   ne correspond pas aux journaux (876 / 618 / 613 µs, ±12 à 31 %) ; les journaux portent
   `tree=ae07d9f` alors que les cas C12–C14 testent `0a2977f`. Refais la campagne sur un
   commit propre et remplace les chiffres par ceux des journaux.
5. `CLAUDE.md` : recompte tout avec les commandes du fichier ; ajoute une ligne « segment
   S1 / plan de vente passerelle » qui explique la coexistence.
6. Étends `tools/check_claims.sh` pour qu'il détecte **ces** dérives (comptes de cas de
   campagne, latence du README contre les journaux, identifiants dupliqués), avec une
   contre-épreuve : casse une ligne, le script doit échouer.

## 4. Phase A : passerelle TCP au niveau d'un pilote payant

Manques relevés par l'audit sur `oasis_mbtcp_gateway` / `oasis_mbtcp_agent` :

1. **Droits par clé, pas seulement des clés.** La configuration TCP porte « des clés, pas
   des permissions » : une clé enregistrée peut écrire dans tous les registres de la carte.
   Aligne-la sur le firmware : attestation d'enrôlement (phase 1.2) avec `ACTUATE`, et une
   **carte de registres par origine** (quel émetteur peut écrire quel registre, quelle
   plage). Spec, tests, 1 harnais Kani (« aucune trame hors de la carte de l'origine »),
   cas de campagne.
2. **k parmi n sur la passerelle.** `oasis-operator-key` n'est qu'une dépendance de test :
   le gateway refuse une liste multi-signée. Rends `OperatorAuthority` accessible au
   binaire, exige des clés distinctes, teste le quorum sur le lien (cas de campagne), et
   mesure le coût.
3. **R14 sur hôte.** `r14_safe` est une constante sur Linux. Soit une source d'état
   (entrées de capteurs configurables, ou l'état de supervision de la partie H), soit
   l'écrire comme **non applicable sur hôte** dans la spec, le README et `CLAUDE.md`. Pas
   de constante silencieuse.
4. **Un ordre à la fois.** Le verrou d'état couvre l'aller-retour automate. Mesure d'abord
   (K=10) le débit avec 2, 5 et 10 IHM concurrentes ; si une IHM qui attend bloque les
   autres, découpe le verrou (décision + journal sous verrou, échange réseau hors verrou)
   en gardant la propriété « un seul site d'écriture ». Test de non-régression sur la
   séquentialité du journal.
5. **Mode dégradé.** Service système (`systemd`), chien de garde, redémarrage automatique,
   état sûr documenté : si la passerelle tombe, l'IHM reçoit 0x0B, jamais un silence ;
   **OASIS n'est jamais dans la chaîne d'arrêt d'urgence** (écris-le dans le guide).
6. **Deux machines, un câble.** Refais la campagne avec PC 1 (IHM + agent) et PC 2
   (passerelle + automate de test écoutant **uniquement** en local). Ajoute les cas :
   câble débranché puis rebranché pendant une écriture, redémarrage de chaque PC, tentative
   d'accès direct à l'automate depuis PC 1 (doit échouer), charge continue de lectures à
   10 Hz pendant les écritures. K=10 sur le vrai lien. Journal relu intact.
7. **Vrai automate.** Si un M221 Ethernet, un WAGO ou un LOGO! est disponible : carte des
   registres réelle, adressage, exceptions renvoyées par l'automate, redémarrage de
   l'automate, 3 exécutions de la campagne. Sinon, écris « non testé sur automate du
   commerce » partout où « automate » apparaît.
8. **Fuzzing** des parseurs ajoutés depuis la dernière campagne (`ORV1` sur TCP, trames de
   lien, configuration), rapportés séparément.
9. **Guide d'installation** pour un intégrateur (`docs/PILOT_INSTALL.md`) : matériel,
   schéma réseau (un seul chemin), fichier de configuration commenté, enrôlement, mise en
   service, lecture du journal, **ce qui n'est pas couvert** (accès physique, clé sur
   disque, autres protocoles).

Livrable : campagne **≥ 50 cas, 3 exécutions, sur deux machines**, latence K=10 sur le
vrai lien, et une ligne de `CLAUDE.md` qui dit exactement ce qui est prouvé et ce qui
ne l'est pas.

## 5. Phase B : la clé ne sort plus (RP2350, puis hôte)

Exécute `prompts/OASIS_KEY_PROTECTION_RP2350.md` tel quel (phases 0 à 4, règle OTP
comprise). En plus, pour la passerelle sur hôte :

1. Clé en fichier : droits `0600`, utilisateur dédié, et **TPM 2.0 si présent** (scellement
   de la graine, signature hors TPM acceptée si le TPM ne fait pas Ed25519 : dis-le).
2. Mesure et écris ce qui reste ouvert sur un Linux standard.
3. Quand B est faite sur E : `CLAUDE.md`, section G **C14**, et le décompte des menaces
   (« 19 sur 23 ») ne changent que si la preuve existe sur carte.

## 6. Phase C : la face embarquée devient démontrable

1. **Une radio émet.** Pilote SX1262 contre le vrai module (pas le mock) : un ordre `OMB1`
   ou `OAS1` traverse un lien LoRa EU868 réel, duty-cycle imposé par le transport,
   budget mesuré contre le calcul (`docs/lora_budget` doit être recodé en Rust ou retiré).
   Écris la portée, le SF, le temps d'antenne mesuré, K=10.
2. **Deuxième famille de puces.** Porte `oasis-silicon-test` sur une cible Cortex-M4
   courante (STM32 ou nRF52), au minimum T0–T6 et un relais v0B à deux cartes. Tailles
   flash/RAM mesurées avec la commande.
3. **Kani complet d'un seul tenant** : sur une machine qui a la mémoire, ou en CI sur
   dépôt public ; la phrase « 182 (ou N) des 189 vérifiés » doit venir d'un run daté.
4. **Trois cartes RP2040 reflashées** avec le firmware courant : le journal des
   modifications (§5 de 1.1.9) est démontré sur silicium, et la non-régression A→B→C
   tourne sur **HEAD**, pas sur un firmware ancien.

## 7. Phase D : CI pour un dépôt privé, sans rien perdre

1. Rapide à chaque envoi (fmt, clippy, tests, MCU, `check_claims`, campagne pilote) ;
   Kani normal **à la demande et hebdomadaire** ; Kani lourd hebdomadaire. Budget mesuré :
   minutes par envoi, et combien d'envois tiennent dans 2 000 minutes par mois.
2. Construction et signature des binaires de livraison (Linux x86-64, ARM64, firmware
   `.uf2`), `SHA256SUMS` signé, SBOM joint, publiés dans un dépôt public séparé
   `oasis-releases` **sans code**. Contrat de licence propriétaire pour les versions
   futures ; le `LICENSE` MIT ne s'applique qu'à ce qui a été publié avant.

## 8. Rapport final attendu

En français simple, avec pour chaque phase :
- ce qui est prouvé, par quoi (test, harnais + contre-épreuve, silicium, campagne) ;
- ce qui ne l'est pas, et pourquoi ;
- les chiffres K=10 ;
- **la section « ce que ma propre vérification a trouvé »**, défauts compris ;
- les trois phrases interdites, toujours absentes ;
- la mise à jour de la section G et de `CLAUDE.md`, recomptée.

Et une dernière page pour un acheteur pilote : **ce qu'il peut attendre, ce qu'il ne doit
pas attendre**, en dix lignes.
