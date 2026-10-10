# Préventes OASIS — GitHub / open source OT, SCADA, Modbus, drones, robotique (2026)

Date de recherche : 2026-10-10. Périmètre : issues/PR/discussions GitHub, Internet-Drafts IETF, forums (PX4, ArduPilot, Victron, Ignition, Home Assistant) où quelqu'un demande des commandes signées/authentifiées, une autorisation de commande, un journal d'audit infalsifiable, une protection anti-rejeu, ou cite une obligation réglementaire (Règlement Machines 1.1.9, CRA, EN 50742, NIS2).

Méthode : lecture directe des pages GitHub (WebFetch), recherche sémantique GitHub (API `search_issues`, scoping par dépôt), recherche web (budget épuisé après ~20 requêtes). Hôtes injoignables depuis le bac à sable : `datatracker.ietf.org`, `www.ietf.org`, miroirs IETF, `docs.px4.io`, `www.cisa.gov`, `app.opencve.io`, `community.victronenergy.com`. Les faits sur ces sources viennent uniquement des extraits de résultats de recherche et sont marqués ⚠️.

**Constat d'ensemble (honnête)** : sur ~30 fils lus, **aucun fil 2026 ne montre un acteur commercial nommé (intégrateur, utility, OEM) qui énonce un besoin ET une date limite ET un budget**. Les signaux de « client payant » sont indirects : (a) un fork d'OpenPLC piloté pour une usine réelle (« Line 4 », passerelle Ignition, document CVSS interne) ; (b) des mainteneurs PX4/ArduPilot qui ouvrent/rouvrent eux-mêmes des issues de signature en sept.–oct. 2026 ; (c) deux Internet-Drafts IETF (juil.–sept. 2026) qui spécifient quasi mot pour mot la passerelle OASIS ; (d) des échéances réglementaires UE (1er nov. 2026 financements onduleurs, janv. 2027 Règlement Machines) sans fil GitHub associé. Les fils les plus proches d'OASIS (autorisation sémantique, jetons de capacité) viennent de **concurrents open source** (SINT Protocol, CleitonQ) et ont été **fermés sans réponse ou refusés** par les mainteneurs — ce qui est en soi une information de marché : les projets autopilote veulent cette couche **hors du code de vol, sur un companion/proxy**, exactement la position d'OASIS.

---

## Liste classée (potentiel client/partenaire, du plus fort au plus faible)

### 1. COG-GTM/OpenPLC_v3 — PR #6 « fix(security): restrict Modbus write and debug function codes to an allowlist »
- URL : https://github.com/COG-GTM/OpenPLC_v3/pull/6 (PR liée #13 « docs(security): Line 4 OpenPLC review report »)
- Date : 27 sept. 2026 — **Statut : ouverte, non fusionnée**
- Projet : fork d'OpenPLC v3 (runtime automate open source), organisation GitHub `COG-GTM` (nature non décrite sur la page)
- Auteur : `devin-ai-integration[bot]`, co-auteur Shawn Azman (rôle/société non affichés)
- Besoin verbatim : titre du constat « Unauthenticated Modbus/TCP writes to Line 4 setpoints (CWE-306 / CWE-862) », CVSS 3.1 8.1 « which the author says is Critical for the site under an internal document » ; « any host on the cell VLAN could rewrite setpoints with plain write requests or use the debugger function code (0x42) to force program variables » ; correctif = « Source-IP allowlist enforced in the Modbus dispatch path for write and debug function codes only » ; rejets journalisés « with the function code and source address » ; déploiement du fichier d'allowlist décrit comme « an operational step for the plant » ; mention d'une « Ignition gateway ».
- Urgence : moyenne-haute (site industriel réel, revue de sécurité interne en cours, rapport de revue « Line 4 » en PR séparée). Pas de date limite ni de règlement cités.
- Pourquoi c'est le n°1 : seul fil 2026 où une **usine réelle** (ligne, VLAN cellule, passerelle Ignition, politique de criticité interne) corrige l'absence d'autorisation d'écriture Modbus. L'allowlist IP est exactement la mesure que la passerelle OASIS dépasse (origine signée Ed25519 au lieu d'une IP usurpable, plage par registre, journal chaîné).
- Réponse FR (3 lignes) :
  « Une allowlist IP sur FC05/06/15/16 est un bon premier pas, mais une IP du VLAN cellule reste usurpable et le journal runtime n'est pas infalsifiable. Nous avons une passerelle Modbus TCP/RTU où chaque écriture est un ordre signé Ed25519 par origine, passé par une porte à 9 conditions (origine enrôlée, non révoquée, non rejouée, dans la plage du registre…), puis journalisé en chaîne hachée ; preuves Kani, `no_std` Rust. Si utile pour la revue "Line 4", nous pouvons partager la spec et un banc d'essai contre votre image OpenPLC. »
- Reply EN :
  "A source-IP allowlist on FC05/06/15/16 is a solid first step, but a cell-VLAN IP is spoofable and the runtime log isn't tamper-evident. We run a Modbus TCP/RTU gateway where every write is an Ed25519 order signed per origin, passed through a 9-condition gate (enrolled, not revoked, not replayed, within per-register range…) and appended to a hash-chained journal; Kani-proved, `no_std` Rust. If helpful for the Line 4 review, happy to share the spec and a test bench against your OpenPLC image."

### 2. PX4/PX4-Autopilot — #27216 « [Bug] Blanket acceptance of MAVLink ADSB_VEHICLE and COLLISION » (+ PR #28697, #28698, #29004 sur la signature)
- URL : https://github.com/PX4/PX4-Autopilot/issues/27216 ; https://github.com/PX4/PX4-Autopilot/pull/28697 ; https://github.com/PX4/PX4-Autopilot/pull/28698 ; https://github.com/PX4/PX4-Autopilot/pull/29004
- Dates : #27216 ouverte 26 avr. 2026 par hamishwillee (docs PX4), auto-fermée 27 août, **rouverte 14 sept. 2026 par julianoes** (« Yes, we should »), auto-assignée — **ouverte**. #28697 (15 sept. 2026, julianoes, approuvée par dakejahl, **ouverte**, backport 1.18 prévu) ; #28698 (15 sept. 2026, brouillon, **ouverte**) ; #29004 (6 oct. 2026, ibondarenko1, **ouverte**).
- Auteurs : mainteneurs PX4 (hamishwillee, julianoes, dakejahl) ; ibondarenko1 (aucune affiliation visible). Aucune société, OEM ou échéance mentionnés sur ces pages.
- Besoin verbatim : #27216 — les messages ADSB_VEHICLE/COLLISION « always allow[ed] », « completely spoofable », « it is a hole that they are not signed » ; proposition : « manufacturers should be encouraged to support signing, and PX4 should require these messages to be signed by default », sinon « lock-by-default ». #28697 — « Receivers on different MAVLink links race on the signing replay table », « Affects every release with signing, starting at v1.18.0-alpha1 », signalé en privé par `lihnucs`. #29004 — checkpoint du timestamp de signature toutes les 60 s, « does not fully prevent replay across restarts ».
- Contexte : CVE-2026-1579 (publiée 31 mars 2026, CWE-306, CVSS 9.8 selon NVD/CISA, 9.3 selon la fiche CNA ; avis CISA ICSA-26-090-02 ; mitigation = activer la signature MAVLink 2) — sources : https://nvd.nist.gov/vuln/detail/cve-2026-1579 ; https://www.cisa.gov/news-events/ics-advisories/icsa-26-090-02 ; https://cveawg.mitre.org/api/cve/CVE-2026-1579. Aucun OEM drone n'a posté de délai sur GitHub/discuss.px4.io dans les résultats obtenus.
- Urgence : moyenne. Travail mainteneur actif sept.–oct. 2026 ; pas de date externe.
- Pourquoi : la table de rejeu partagée, le timestamp persistant et la « signature par défaut » sont les trois problèmes qu'OASIS v0B résout déjà (fenêtre compteur 128 bits persistée en flash, bail émetteur après reboot, révocation imposée au relais). Partenaire technique potentiel plus que client.
- Réponse FR : « Nous avons buté sur les mêmes trois points (rejeu après redémarrage, table de rejeu partagée entre liens, messages "toujours acceptés") sur un maillage multi-sauts : notre couche v0B signe `réseau‖origine‖compteur‖SHA-256(charge)` en Ed25519 par origine, persiste la fenêtre de compteur en flash et a été rejouée/coupée au secteur sur 3 RP2040 sans acceptation. Spec et bancs publics ; si une couche d'autorisation hors du code de vol (companion) intéresse PX4 pour ADSB/COLLISION, nous pouvons proposer un PoC. »
- Reply EN : "We hit the same three issues (post-reboot replay, replay table shared across links, always-accepted messages) on a multi-hop mesh: our v0B layer signs `network‖origin‖counter‖SHA-256(payload)` with per-origin Ed25519, persists the counter window to flash, and survived replay/power-cut tests on 3× RP2040 with zero acceptances. Spec and benches are public; if an off-flight-code (companion) authorization layer for ADSB/COLLISION is of interest, we can propose a PoC."

### 3. IETF — draft-morrison-ot-command-authority (-00 juil. 2026 → -03 sept. 2026) et draft-das-ot-actuation-finality-00 (août 2026) ⚠️
- URL : https://datatracker.ietf.org/doc/draft-morrison-ot-command-authority/ ; https://www.ietf.org/archive/id/draft-morrison-ot-command-authority-03.html ; https://datatracker.ietf.org/doc/html/draft-das-ot-actuation-finality-00 ; https://www.ietf.org/archive/id/draft-liu-iotops-modbus-seriallink-sec-spec-07.txt
- Dates : Morrison -00 juil. 2026, -03 sept. 2026 (3 auteurs) ; Das -00 août 2026 ; Liu -07 août 2026 — **tous actifs, soumissions individuelles, aucun n'est RFC**.
- Auteurs/affiliations : **non vérifiés** (pages injoignables) — gap.
- Besoin verbatim (extraits de résultats de recherche) : Morrison « refuses a control action issued to an OT or industrial control system on a software agent's authority unless it carries verifiable proof of the agent's identity, the human principal it acts for, that principal's authorisation of the specific action, and an append-only audit record. It fails closed on authority, never on safety, and must not be placed in the trip path of a safety function. » Das : « Modbus is often a raw register write with no act object at all », « a valid signed call can still be the wrong act » ; profil liant « device, tag, value, mode, and zone into a digest », autorité « single-use and tied to a specific sink ».
- Urgence : moyenne (révisions rapides : 4 versions en 3 mois). Aucun implémenteur ni intérêt de liste de diffusion trouvé (recherche ciblée impossible, budget épuisé).
- Pourquoi : correspondance quasi 1:1 avec OASIS (ordre `OMB1` lié à unité+registre+valeur, `cmd_seq` à usage unique, journal append-only, porte d'actuation hors chemin de sécurité). Positionnement = **première implémentation `no_std` vérifiée** d'un profil IETF en cours ; partenaires = les auteurs.
- Réponse FR : « Votre profil (preuve d'identité + principal + autorisation de l'acte précis + journal append-only, fail-closed sur l'autorité et jamais sur la sécurité) correspond à ce que nous avons implémenté en Rust `no_std` : ordre lié à `unité‖registre‖valeur`, `cmd_seq` à usage unique, journal chaîné, porte d'actuation à 9 conditions hors chemin de déclenchement. Preuves Kani et essais sur silicium (3× RP2040, coupures secteur) publics. Intéressés pour une section "Implementation Status" ou un test d'interop ? »
- Reply EN : "Your profile (proof of identity + principal + authorization of the specific act + append-only audit, fail-closed on authority never on safety) matches what we built in `no_std` Rust: order bound to `unit‖register‖value`, single-use `cmd_seq`, hash-chained journal, 9-condition actuation gate outside the trip path. Kani proofs and silicon tests (3× RP2040, power cuts) are public. Interested in an Implementation Status entry or an interop test?"

### 4. ArduPilot/ardupilot — #34489 « MAVLink2 signing: unsigned packets accepted on channel 0 when channel 0 is a network link (Linux boards) »
- URL : https://github.com/ArduPilot/ardupilot/issues/34489
- Date : 24 sept. 2026 — **ouverte** ; 1 commentaire (peterbarker, 28 sept. : « you _could_ just configure that first port to be something on 127.0.0.1 or similar, correct? »)
- Auteur : Nikthenik (académique : « evaluating post-quantum key establishment over MAVLink for an academic publication »), Parrot Bebop 1 sous Linux, ArduCopter 4.7.0-dev.
- Besoin verbatim : commentaire code « always accept channel 0, assumed to be secure channel » ; « On Linux boards, however, channel 0 can be the network link to the ground station » ; capture : 6 `COMMAND_LONG` non signés acquittés en 15–37 ms ; correctifs proposés : exemption limitée aux ports filaires, configurable et désactivée par défaut sur réseau.
- Urgence : basse-moyenne (recherche, pas de client). Statut : non résolu.
- Fils ArduPilot connexes 2026 : #33604 (30 juin 2026, rmackay9, **ouverte**, signature sur USB pour protéger paramètres/logs/missions/scripts Lua ; aucune société) ; #33416 (12 juin 2026, rmackay9, **ouverte**, rejet des commandes hors spec — « this is a robustness issue, not a security issue ») ; #13860 (2020, fermée, hors périmètre).
- Réponse FR : « L'exception "canal 0 = sûr" montre la limite d'une signature HMAC à clé partagée par lien : la confiance est attachée au port, pas à l'émetteur. Une signature par origine (Ed25519) avec compteur persistant retire ce choix de configuration : un paquet non signé ou rejoué est refusé quel que soit le canal, y compris sur Wi-Fi/UDP Linux. Nous avons mesuré ~125 µs de vérification sur Cortex-M0+ ; volontiers pour comparer avec votre banc Bebop. »
- Reply EN : "The 'channel 0 is secure' exemption shows the limit of per-link shared-key HMAC signing: trust is bound to the port, not the sender. Per-origin Ed25519 signing with a persisted counter removes that configuration choice: an unsigned or replayed packet is refused on any channel, Linux Wi-Fi/UDP included. We measured ~125 µs verify on Cortex-M0+; happy to compare against your Bebop bench."

### 5. OpenEMS/openems — #3972 « PV-Inverter ActivePowerLimit goes negative & is written unchecked to unsigned Modbus registers »
- URL : https://github.com/OpenEMS/openems/issues/3972
- Date : 10 sept. 2026, **fermée 19 sept. 2026** (PR #3973, « clamp setpoint to 0 », par sfeilmeier)
- Projet : OpenEMS (EMS open source porté par FENECON) ; auteur JonasBechler (société non visible)
- Besoin verbatim : « A PV-Inverter power limit can go below zero inside OpenEMS » ; pilotes Huawei PV Inverter et SmartLogger écrivent sans contrôle de plage dans les registres non signés 40235/40424 ; exemple −500 W → ≈ 6 553,1 kW ; conséquence : l'onduleur « can run unlimited exactly when a limit is needed », violation de la limite d'injection réseau. Pilotes SunSpec (Kaco, SMA, Fronius, SolarEdge, Kostal, Solarlog) non affectés (clamp 1–100 %).
- Urgence : résolue côté OpenEMS ; mais illustre le besoin d'une **borne par registre indépendante de l'application** (condition « dans les limites » de la porte OASIS, garde NaN incluse).
- Fil connexe : #3815 (4 juil. 2026, fermé 27 juil., jankaifer) « Victron adapter is writing to modbus even in read-only mode » — « it wrote something to modbus anyway and it stopped my solar from coming it and it disallowed the battery to charge ».
- Note commerciale : la doc evcc indique que l'écriture Modbus sur FENECON FEMS exige une licence payante « FEMS App Modbus/TCP Write Access » (https://docs.evcc.io/en/meters/fenecon-modbus-api) — un OEM EMS monétise déjà l'**autorisation d'écriture**.
- Réponse FR : « Le clamp applicatif corrige ce chemin, mais un autre contrôleur ou un bug futur peut réécrire un registre non signé hors plage. Une porte indépendante côté passerelle — plage par registre configurée, refus avant construction de la trame, journal de chaque décision — bloque la classe entière (nous testons justement la valeur hors plage et le NaN). Nous avons un banc OpenEMS → passerelle → `rmodbus` ; intéressés par un essai sur un pilote Huawei ? »
- Reply EN : "The app-side clamp fixes this path, but another controller or a future bug can still write an out-of-range value to an unsigned register. An independent gate at the gateway — configured per-register range, refusal before the frame is built, every decision journaled — blocks the whole class (we test exactly out-of-range and NaN). We have an OpenEMS → gateway → `rmodbus` bench; interested in a run against a Huawei driver?"

### 6. thiagoralves/OpenPLC_v3 — #324 « Security Feature request: optional write protection for the Modbus TCP server »
- URL : https://github.com/thiagoralves/OpenPLC_v3/issues/324
- Date : 31 mars 2026 — **ouverte, 0 commentaire ; dépôt archivé (lecture seule) le 4 avr. 2026**
- Auteur : APEvul-cyber (aucune société)
- Besoin verbatim : le serveur « accepts write function codes such as FC05, FC06, FC15, and FC16 from any client that can reach the Modbus TCP port » ; demande d'un mode lecture seule et du blocage par code fonction, « for deployments where Modbus is not confined to a fully trusted control network ».
- Urgence : basse (dépôt archivé, pas de réponse). Utile comme preuve de demande récurrente (voir aussi n°1).
- Réponse FR : « Le mode lecture seule/par code fonction est binaire : soit personne n'écrit, soit tout le réseau écrit. Notre passerelle autorise l'écriture par origine signée et par registre (FC06/FC16 ≤ 8 regs), avec rejeu et révocation gérés, et ne construit la trame RTU/TCP qu'après décision `Act`. Si le fork actif d'OpenPLC vous intéresse, nous avons un banc prêt. »
- Reply EN : "Read-only or per-function-code modes are binary: either nobody writes or the whole network does. Our gateway authorizes writes per signed origin and per register (FC06/FC16 ≤ 8 regs), with replay and revocation handled, and only builds the RTU/TCP frame after an `Act` decision. If you're on the active OpenPLC fork, we have a bench ready."

### 7. alexlenk/ha-dev-tools — #76 « Tooling gap: no way to safely test/debug Modbus writes without ad hoc automations »
- URL : https://github.com/alexlenk/ha-dev-tools/issues/76
- Date : 23 sept. 2026, **fermée 28 sept. 2026** (v2.19.0, PR #78 ; `call_service` générique « deliberately left out as too broad »)
- Auteur : alexlenk (propriétaire du dépôt, particulier) — onduleur 12 kW, batterie, passerelle Waveshare RS485→PoE, EMHASS.
- Besoin verbatim : « this is specifically a home instance with an inverter, a battery, and physical devices (locks, garage door, etc.) on it » ; « inverter register writes can affect battery charge/discharge behavior » ; option 4 « write_register … should use propose/confirm confirmation at minimum » ; demande explicite d'une « security review » avant de choisir.
- Urgence : résolue ; marché résidentiel (faible valeur), mais formulation exacte du besoin « propose/confirm + historique des transactions par registre » = ordre signé + journal.
- Réponse FR : « Le "propose/confirm" que vous décrivez est une autorisation par écriture : nous l'implémentons côté passerelle (ordre signé lié au registre et à la valeur, usage unique, journal chaîné des décisions), ce qui donne aussi l'historique par registre que vous cherchiez. Léger (`no_std`), testable contre `rmodbus`. »
- Reply EN : "The propose/confirm you describe is per-write authorization: we implement it at the gateway (signed order bound to register and value, single-use, hash-chained decision journal), which also yields the per-register history you were after. Lightweight (`no_std`), testable against `rmodbus`."

### 8. pshkv — PX4 #26972 et ArduPilot #32681 « RFC: … command authorization layer for AI agents — beyond MAVLink message signing » (SINT Protocol)
- URL : https://github.com/PX4/PX4-Autopilot/issues/26972 ; https://github.com/ArduPilot/ardupilot/issues/32681 ; https://github.com/pshkv/sint-protocol
- Dates : 4 avr. 2026 (les deux). PX4 : **fermée « not planned » par le bot le 4 août 2026, zéro commentaire humain**. ArduPilot : **fermée le 5 avr. 2026 par peterbarker**.
- Auteur : pshkv (individu, pas de société ; projet Apache-2.0, TypeScript, 11 étoiles ; jetons de capacité Ed25519, registre d'audit chaîné SHA-256, ponts ROS 2/MAVLink/OPC UA/MQTT Sparkplug ; **pas de Modbus**).
- Besoin verbatim : « CVE-2026-1579 (CVSS 9.8) demonstrated that MAVLink's lack of mandatory cryptographic signing is a critical vulnerability » ; la signature répond seulement à « is this message from a trusted sender? » ; « PX4 sees only the commands that SINT has authorized ».
- Réponses des mainteneurs ArduPilot (verbatim) : khancyr « don't use probabilistic generator to control a real vehicle » ; peterbarker « we trust our GCSs, especially when they have the signing key for your vehicle » (« just some form of proxy service to vet input to the autopilot ») ; rmackay9 : mieux placé sur un companion (RPanion, BlueOS), « we need to be careful that we don't add to the flight code unless it's really necessary », mais ouverts à des contrôles d'acceptation/rejet « if the problem is clearly defined ».
- Urgence : nulle côté projets ; **concurrent/partenaire** direct (même thèse, sans Modbus, sans `no_std`, sans preuves formelles).
- Réponse FR (à poster sous #32681 ou en issue SINT) : « Même diagnostic : la signature authentifie l'émetteur, pas l'acte. Nous l'avons porté côté OT (Modbus TCP/RTU) en Rust `no_std` avec preuves Kani : ordre signé par origine, porte à 9 conditions, journal chaîné, révocation signée, le tout hors du code de vol comme le suggère rmackay9. Comparer nos formats d'ordre/jeton vous intéresse ? »
- Reply EN: "Same diagnosis: signing authenticates the sender, not the act. We took it to the OT side (Modbus TCP/RTU) in `no_std` Rust with Kani proofs: per-origin signed order, 9-condition gate, hash-chained journal, signed revocation, all outside flight code as rmackay9 suggests. Interested in comparing our order/token formats?"

### 9. mavlink/mavlink — #2525 « Post-quantum authenticated COMMAND_LONG / SET_POSITION_TARGET: does MAVLink have a PQC roadmap? » (+ ArduPilot #33485)
- URL : https://github.com/mavlink/mavlink/issues/2525 ; https://github.com/ArduPilot/ardupilot/issues/33485
- Dates : 16 juin 2026, **fermée 30 juin 2026** (hamishwillee, après dev call du 1er juil. : « We don't have a post quantum roadmap and we don't think we need it »). ArduPilot #33485 : 18 juin 2026, fermée le jour même « not planned » (tpwrules : « an LLM-extruded advertisement »).
- Auteur : cleitonaugusto (CleitonQ, Rust, ML-KEM-1024/ML-DSA-87) ; intervenant x0tta6bl4-ai propose « hybrid ML-DSA-44 signing (now to 2027) », ML-KEM-768 2027+, PQC-only 2030+ ; NSA CNSA 2.0 cité (2030 défense).
- Verbatim mainteneur : « MAVLink does not support encryption (only signing), so everything is public. »
- Urgence : nulle côté MAVLink WG. Signal : OASIS Phase 1.1 (autorité hybride Ed25519 + ML-DSA-44, 377 ms sur RP2040) couvre ce que le WG refuse — différenciateur pour les acheteurs défense/CNSA 2.0, pas un fil client.
- Réponse FR : « Nous partageons l'avis du WG qu'un PQC par paquet n'a pas de sens en bande étroite ; nous l'avons limité aux messages d'autorité basse fréquence (révocation, politique, enrôlement, firmware) en hybride Ed25519 + ML-DSA-44 — 377 ms et 48,7 Ko de pile sur RP2040, chiffres publics. Les ordres restent classiques. »
- Reply EN: "We agree with the WG that per-packet PQC makes no sense on narrowband links; we confined it to low-frequency authority messages (revocation, policy, enrollment, firmware) as hybrid Ed25519 + ML-DSA-44 — 377 ms and 48.7 KB stack on RP2040, numbers public. Orders stay classical."

### 10. meshtastic/firmware — #10971 « [2.8.0] Authorize signed plaintext remote admin in licensed mode » et #11247 « restrict remote LocalStats requests to configured admin keys »
- URL : https://github.com/meshtastic/firmware/issues/10971 ; https://github.com/meshtastic/firmware/issues/11247
- Dates : 10 juil. 2026 → **fermée « not planned » (stale) 13 sept. 2026** ; 27 juil. 2026 → **fermée « not planned » (stale) 23 sept. 2026**
- Auteur : RCGV1 (contributeur) ; aucune société/déploiement.
- Besoin verbatim : admin distant en mode licencié (radioamateur, clair) accepté seulement si « verified XEdDSA signature, and the signer key matches a populated admin_key slot » ; « admin_key is treated as authorization, not discovery » ; réponse `NOT_AUTHORIZED` pour requêtes non autorisées.
- Urgence : nulle (abandon). Contexte CVE : CVE-2025-55292 (janv. 2026, identité liée au MAC, correctif 2.7.6) et CVE-2024-47079 (module matériel distant, CWE-345).
- Réponse FR : « L'autorisation par clé admin + signature XEdDSA sans compteur persistant reste rejouable après redémarrage ; nous avons résolu ce point sur maillage LoRa-like avec une fenêtre de compteur 128 bits persistée et un bail émetteur. Si la 2.8 reprend le sujet, nos vecteurs de test (rejeu après coupure secteur) sont réutilisables. »
- Reply EN: "Admin-key authorization plus XEdDSA signature without a persisted counter is still replayable after a reboot; we solved that on a LoRa-like mesh with a persisted 128-bit counter window and a sender lease. If 2.8 picks this back up, our test vectors (post-power-cut replay) are reusable."

### 11. Rapports de vulnérabilité sur piles Modbus (demande implicite de pile durcie)
- stephane/libmodbus #874 « Unauthenticated remote DoS via forced 500 ms block in modbus_reply() error paths » — https://github.com/stephane/libmodbus/issues/874 — 22 sept. 2026, Synmac05, **ouverte, 0 commentaire**, CVSS 7.5 ; verbatim : « Unauthenticated, network-reachable, trivial to trigger, and directly degrades availability. » Pas de demande d'authentification.
- debevv/nanoMODBUS #122 « Unauthenticated heap/stack OOB write in nanomodbus FC 0x14 » — https://github.com/debevv/nanoMODBUS/issues/122 — 12 juil. 2026, entropy1337, **ouverte** ; 1 commentaire (jacobq, 10 sept. : désactiver via `NMBS_SERVER_READ_FILE_RECORD_DISABLED`) ; pas de réponse mainteneur.
- VOLTTRON/volttron #3366 « Serialize access to each Modbus device in the modbus_tk driver » — https://github.com/VOLTTRON/volttron/issues/3366 — 7 oct. 2026, craigpnnl, **ouverte** ; concurrence lecture/écriture, pas d'autorisation.
- CVE 2026 automates Modbus sans authentification : Delta DVP-12SE CVE-2026-12819 (CWE-306, CVSS 4.0 9.3, avis Delta PCSA-2026-00011) https://cvefeed.io/vuln/detail/CVE-2026-12819 ; GPL Odorizers GPL750 CVE-2026-4436 (écriture de registres d'injection d'odorant, CVSS 8.6) https://securityarsenal.com/blog/cve-2026-4436-gpl-odorizers-gpl750-unauthenticated-modbus-exploitation-detection-and-defense ; Welker PLC CVE-2026-24790 (ICSA-26-050-04).
- Urgence/valeur : faible comme prospects ; utiles comme **argumentaire** (parseurs totaux prouvés par Kani vs OOB/DoS dans libmodbus/nanoMODBUS ; « dernier chemin » devant un DVP-12SE).
- Réponse FR (libmodbus/nanoMODBUS) : « Les deux rapports illustrent qu'une pile Modbus exposée sans authentification est attaquable même sans écriture. Notre passerelle place une porte signée devant l'esclave et ses parseurs sont prouvés totaux (Kani) ; nous pouvons partager les harnais si utile pour ajouter des tests différentiels. »
- Reply EN: "Both reports show an exposed, unauthenticated Modbus stack is attackable even without writes. Our gateway puts a signed gate in front of the slave and its parsers are proven total (Kani); happy to share the harnesses if useful for differential tests."

### 12. Hors périmètre / faux positifs vérifiés
- ripplebiz(meshcore-dev)/MeshCore discussion #1736 « RFC: Reticulum Network Stack as a Decentralized Backhaul Layer for MeshCore » — https://github.com/ripplebiz/MeshCore/issues/1736 — 18 févr. 2026, GrayHatGuy, active jusqu'au 21 mai 2026 ; **ne porte pas sur l'autorisation de commande** (backhaul/pontage). Une recherche « signed command authorization » dans meshcore-dev/MeshCore ne renvoie qu'une issue de preset réglementaire US (#945, oct. 2025).
- CIRISAI/CIRISAgent #402 « Critical Security Fix: Discord WA validation… » — https://github.com/CIRISAI/CIRISAgent/issues/402 — PR fusionnée le 28 août **2025** ; validation d'identité Discord, pas de commande OT.
- ArduPilot #13860 (2020, fermé) : rejeu via GPS spoofing du timestamp de signature ; aucun commentaire 2026.
- Recherches sans résultat 2026 pertinent : pymodbus (seul #2850, janv. 2026, divulgation privée, fermé), tokio-modbus (#133 TLS, 2022), eclipse-4diac/4diac-forte (0), OpenFMB (0), ScadaBR (0), Node-RED modbus (0), Home Assistant core (0 sur auth/audit Modbus ; #167057 verrou inter-hubs), evcc (121 issues Modbus 2026, aucune sur auth/audit), markqvist/Reticulum (0), Ignition forum (fils audit 2–5 ans, aucun sur signature/infalsifiabilité).

---

## Question clé 1 — PX4 #29004 / CVE-2026-1579 : qui demande l'authentification des commandes ? OEM avec échéance ?

### Takeaway
#29004 est une PR technique (persistance du timestamp de signature) d'un contributeur sans affiliation ; la seule demande d'« autorisation de commande » post-CVE (#26972, SINT) est restée sans aucune réponse humaine et a été fermée par le bot. Aucun OEM drone ni échéance n'apparaît sur GitHub ou dans les résultats sur discuss.px4.io ; en revanche les mainteneurs eux-mêmes poussent la signature par défaut (#27216 rouverte le 14 sept. 2026).

### Cited Findings
- #29004 « fix(mavlink): persist signing timestamp checkpoints », ouverte 6 oct. 2026 par ibondarenko1, ouverte, 0 revue humaine, « does not fully prevent replay across restarts and is not a replacement for #28697 or #28698 » ; seul nom de société : « Assisted-by: OpenAI:Codex » — [PX4 PR #29004](https://github.com/PX4/PX4-Autopilot/pull/29004)
- #28697 (15 sept. 2026, julianoes, approuvée dakejahl, ouverte) : « Receivers on different MAVLink links race on the signing replay table », « Affects every release with signing, starting at v1.18.0-alpha1 », signalé en privé par lihnucs — [PX4 PR #28697](https://github.com/PX4/PX4-Autopilot/pull/28697)
- #28698 (15 sept. 2026, brouillon, « Need to test it », « Assisted-by: Claude ») — [PX4 PR #28698](https://github.com/PX4/PX4-Autopilot/pull/28698)
- #27216 (26 avr. 2026, hamishwillee) : ADSB_VEHICLE/COLLISION « completely spoofable », « PX4 should require these messages to be signed by default » ; rouverte par julianoes 14 sept. 2026 (« Yes, we should ») — [PX4 #27216](https://github.com/PX4/PX4-Autopilot/issues/27216)
- #26972 (4 avr. 2026, pshkv) cite « CVE-2026-1579 (CVSS 9.8) » ; fermée « not planned » 4 août 2026 ; « There were no human comments » — [PX4 #26972](https://github.com/PX4/PX4-Autopilot/issues/26972)
- CVE-2026-1579 : publiée 31 mars 2026, CWE-306, PX4 v1.16.0 SITL ; CVSS 9.8 (NVD/CISA) vs 9.3 (fiche CNA) ; avis CISA ICSA-26-090-02 ; mitigation : signature MAVLink 2 sur tous les liens non-USB — [NVD](https://nvd.nist.gov/vuln/detail/cve-2026-1579), [CISA](https://www.cisa.gov/news-events/ics-advisories/icsa-26-090-02), [MITRE](https://cveawg.mitre.org/api/cve/CVE-2026-1579)
- Autres issues sécurité MAVLink PX4 2026 : #28349 (23 août 2026, REYu6, contournement de la whitelist d'écriture MAVLink FTP, fermée), #28347 (OOB read MAVLink FTP, fermée) — [PX4 #28349](https://github.com/PX4/PX4-Autopilot/issues/28349)

### Inferences
- La dynamique PX4 est « signature par défaut + corrections de robustesse de la signature », pas « couche d'autorisation ». Un partenariat crédible passe par un companion/proxy, pas par le code de vol.
- L'absence totale de réaction à #26972 suggère que le canal GitHub PX4 n'est pas celui où les OEM expriment des besoins ; les demandes passent probablement en privé (cf. rapport privé de lihnucs).

### Gaps
- Aucun fil discuss.px4.io sur la CVE trouvé (recherche web épuisée avant une requête ciblée `discuss.px4.io`).
- Affiliations de julianoes/dakejahl/ibondarenko1 non affichées ; non supposées.
- Crédit du chercheur de la CVE (un résultat pointe « Dolev Aviv » sur dbugs.ptsecurity.com) non vérifié : pages CISA/OpenCVE injoignables.

## Question clé 2 — ArduPilot #13860 et discussions signature MAVLink 2026

### Takeaway
#13860 est clos depuis 2020 sans activité 2026. En 2026, les fils ArduPilot pertinents sont #34489 (contournement de la signature sur canal 0 réseau, ouvert), #33604 (signature sur USB, mainteneur, ouvert), et deux RFC externes (#32681 SINT, #33485 PQC) refusées en ≤ 1 jour avec une position claire : la vérification des commandes appartient à un proxy/companion.

### Cited Findings
- #13860 : ouvert 22 mars 2020, fermé 26 mars 2020 par peterbarker ; tridge : « We force the 64 bit timestamp to only ever go forward » ; aucun commentaire 2026 — [ArduPilot #13860](https://github.com/ArduPilot/ardupilot/issues/13860)
- #34489 (24 sept. 2026, Nikthenik, ouvert) : « always accept channel 0, assumed to be secure channel » ; « On Linux boards, however, channel 0 can be the network link to the ground station » ; Parrot Bebop 1 ; peterbarker 28 sept. : « you _could_ just configure that first port to be something on 127.0.0.1 » — [ArduPilot #34489](https://github.com/ArduPilot/ardupilot/issues/34489)
- #33604 (30 juin 2026, rmackay9, ouvert) : protéger « parameters, log files, mission commands, fences, rally points, lua scripts » via signature sur USB — [ArduPilot #33604](https://github.com/ArduPilot/ardupilot/issues/33604)
- #33416 (12 juin 2026, rmackay9, ouvert) : « Note this is a robustness issue, not a security issue. » — [ArduPilot #33416](https://github.com/ArduPilot/ardupilot/issues/33416)
- #32681 (4 avr. 2026, pshkv, fermé 5 avr.) : peterbarker « we trust our GCSs, especially when they have the signing key for your vehicle » ; rmackay9 : companion computer (RPanion, BlueOS), « we need to be careful that we don't add to the flight code unless it's really necessary » — [ArduPilot #32681](https://github.com/ArduPilot/ardupilot/issues/32681)
- #33485 (18 juin 2026, cleitonaugusto, fermé le jour même) : tpwrules « an LLM-extruded advertisement » ; auteur demande « whether any customers or government programmes already require quantum-era C2 security » — sans réponse — [ArduPilot #33485](https://github.com/ArduPilot/ardupilot/issues/33485)
- #33253 (30 mai 2026, fermé 5 juin) : DoS distant non authentifié via PREFLIGHT_REBOOT_SHUTDOWN en builds de production — [ArduPilot #33253](https://github.com/ArduPilot/ardupilot/issues/33253)
- mavlink/mavlink #2525 (16–30 juin 2026) : hamishwillee « MAVLink does not support encryption (only signing), so everything is public » ; dev call 1er juil. : « We don't have a post quantum roadmap and we don't think we need it » — [mavlink #2525](https://github.com/mavlink/mavlink/issues/2525)

### Inferences
- Les mainteneurs ArduPilot acceptent explicitement l'idée d'un filtre d'acceptation/rejet des commandes **si le problème est clairement défini** et placé sur companion : porte d'entrée pour une démo OASIS (porte d'actuation + journal) sur RPanion/BlueOS.
- Le rejet rapide des RFC « générées par LLM » impose un ton factuel, bancs chiffrés, pas de pitch.

### Gaps
- Aucun fil discuss.ardupilot.org 2026 lu (recherche web épuisée) ; le fil « user access control » cité par rmackay9 dans #32681 n'a pas pu être ouvert.

## Question clé 3 — libmodbus #874, nanoMODBUS #122, pymodbus, modbus-tk, rodbus, tokio-modbus, FreeModbus, OpenPLC, ScadaBR, Node-RED, Ignition, OpenFMB, 4diac, Home Assistant : qui demande une autorisation d'écriture ou un journal d'audit en 2026 ?

### Takeaway
Deux demandes explicites d'autorisation d'écriture Modbus en 2026 : OpenPLC upstream #324 (mars, sans réponse, dépôt archivé) et le fork COG-GTM/OpenPLC_v3 PR #6 (sept., usine réelle « Line 4 », allowlist IP + journalisation des rejets). libmodbus #874 et nanoMODBUS #122 sont des rapports de vulnérabilité sans demande d'authentification. Rien de pertinent sur pymodbus, tokio-modbus, 4diac, OpenFMB, ScadaBR, Node-RED, Home Assistant core ni le forum Ignition en 2026.

### Cited Findings
- COG-GTM/OpenPLC_v3 PR #6, 27 sept. 2026, ouverte : « Unauthenticated Modbus/TCP writes to Line 4 setpoints (CWE-306 / CWE-862) », CVSS 8.1, « Critical for the site under an internal document », « any host on the cell VLAN could rewrite setpoints », allowlist IP « for write and debug function codes only », rejets journalisés, déploiement = « an operational step for the plant », mention d'une « Ignition gateway » — [COG-GTM PR #6](https://github.com/COG-GTM/OpenPLC_v3/pull/6)
- thiagoralves/OpenPLC_v3 #324, 31 mars 2026, ouvert, 0 commentaire, dépôt archivé 4 avr. 2026 : « accepts write function codes such as FC05, FC06, FC15, and FC16 from any client that can reach the Modbus TCP port » — [OpenPLC #324](https://github.com/thiagoralves/OpenPLC_v3/issues/324)
- libmodbus #874, 22 sept. 2026, ouvert, 0 commentaire, CVSS 7.5 : « Unauthenticated, network-reachable, trivial to trigger » — [libmodbus #874](https://github.com/stephane/libmodbus/issues/874)
- nanoMODBUS #122, 12 juil. 2026, ouvert : OOB write FC 0x14 ; jacobq 10 sept. : « Can this be mitigated by defining NMBS_SERVER_READ_FILE_RECORD_DISABLED? » — [nanoMODBUS #122](https://github.com/debevv/nanoMODBUS/issues/122)
- ha-dev-tools #76, 23–28 sept. 2026, fermé : « inverter register writes can affect battery charge/discharge behavior », « propose/confirm confirmation at minimum » — [ha-dev-tools #76](https://github.com/alexlenk/ha-dev-tools/issues/76)
- VOLTTRON #3366, 7 oct. 2026, ouvert, craigpnnl : sérialisation par appareil, pas d'autorisation — [VOLTTRON #3366](https://github.com/VOLTTRON/volttron/issues/3366)
- pymodbus #2850 (27 janv. 2026, fermé) : « Regarding a potential vulnerability disclosure in pymodbus », 2 commentaires — [pymodbus #2850](https://github.com/pymodbus-dev/pymodbus/issues/2850)
- Forum Ignition : fils audit (« Ignition Audit trail », « Audit Events in Ignition 8.1.18 », inondation du journal par `system.tag.writeBlocking` ~100 Go/jour) datés 2–5 ans ; rien sur signature ou infalsifiabilité — [forum.inductiveautomation.com](https://forum.inductiveautomation.com/t/ignition-audit-trail/63962)
- Recherche sémantique GitHub « Modbus command authorization signed orders tamper-evident audit journal gateway NIS2 CRA Machinery Regulation 2026 » : 0 résultat ; « SunSpec Modbus write authentication inverter security RBAC TLS » : 0 résultat ; eclipse-4diac/4diac-forte : 0 — (API GitHub search_issues, 10 oct. 2026)

### Inferences
- Le marché exprime le besoin via des rapports de vulnérabilité (CWE-306) plutôt que via des demandes de fonctionnalité : le discours de vente doit partir de la CWE-306 et du « dernier chemin » devant l'esclave.
- Le fork COG-GTM montre un intégrateur/exploitant qui bricole lui-même la mesure ; c'est le profil « client pilote » le plus réaliste trouvé.

### Gaps
- Nature de COG-GTM (intégrateur ? exploitant ?) et identité de Shawn Azman non vérifiées.
- FreeModbus, modbus-tk, rodbus, ScadaBR, OpenFMB : pas de dépôt interrogé avec succès (budget) ; rien trouvé via la recherche globale.

## Question clé 4 — SunSpec, SMA, Fronius, Huawei, Victron, OpenEMS, EVCC, SolarAssistant : sécurisation des écritures onduleurs, Lituanie, restrictions UE 1er nov. 2026

### Takeaway
Aucun fil 2026 ne relie explicitement l'obligation lituanienne ou les restrictions UE à une demande d'écriture Modbus sécurisée. Les signaux sont : OpenEMS #3972 (écriture hors plage vers Huawei, sept. 2026), OpenEMS #3815 (écriture en mode lecture seule sur Victron), une licence payante FENECON pour l'écriture Modbus, et des projets SunSpec « Secure Modbus » (mTLS + RBAC) en revue jusqu'au 1er août 2026. La date du 1er nov. 2026 est une échéance de **financement UE** (grandfathering), pas une interdiction technique.

### Cited Findings
- OpenEMS #3972 (10→19 sept. 2026, fermé) : limite négative écrite dans les registres non signés Huawei 40235/40424, « −500 W » → « ≈ 6553.1 kW » ; correctif « clamp setpoint to 0 » — [OpenEMS #3972](https://github.com/OpenEMS/openems/issues/3972)
- OpenEMS #3815 (4→27 juil. 2026, fermé) : « it wrote something to modbus anyway and it stopped my solar… » — [OpenEMS #3815](https://github.com/OpenEMS/openems/issues/3815)
- evcc docs : « for active battery control on FENECON FEMS systems, a commercial license (FEMS App Modbus/TCP Write Access) is required » — [docs.evcc.io](https://docs.evcc.io/en/meters/fenecon-modbus-api)
- SunSpec : « Secure SunSpec Modbus Test Procedures draft moved to TEST status », mTLS + RBAC, revue publique jusqu'au 1er août 2026 — [SunSpec mid-year update](https://sunspec.org/events/sunspec-webinar-register-now-mid-year-standards-update/)
- FCC : 29 juil. 2026, onduleurs étrangers ajoutés à la Covered List (mesure US) — [SunSpec white paper](https://sunspec.org/wp-content/uploads/2009/03/How-SunSpec-Certifications-Can-Support-FCC-Compliance-White-Paper.pdf)
- Lituanie, art. 733 : interdit la gestion à distance par des fabricants de pays jugés menaçants (>100 kW), applicable 1er mai 2025, existant au 1er mai 2026 (échéance **passée**) ; « Grid operators shall not connect… if the security of their control systems does not meet these requirements » — [pv-magazine](https://www.pv-magazine.com/?p=39019), [pv-tech](https://pv-tech.org/lithuania-to-block-chinese-inverters-with-cybersecurity-legislation), [ESMC](https://esmc.solar/lithuanian-parliament-bans-remote-access-of-companies-from-china-to-lithuanian-solar-wind-and-storage-devices/)
- UE : restriction des financements EIB/EIF pour projets avec onduleurs « high-risk » ; projets notifiés avant le 1er mai 2026 doivent être « ripe for Commission approval by 1 November 2026 to benefit from grandfathering. Grandfathering does not exempt from cybersecurity measures » ; phase stricte à partir d'avril 2027 ; BESS PCS inclus — [CMS](https://cms.law/en/hun/legal-updates/eu-moves-to-restrict-eu-backed-financing-for-projects-using-high-risk-inverters), [ESS News](https://www.ess-news.com/2026/05/04/eu-funding-ban-on-high-risk-inverters-including-chinese-suppliers-extends-to-bess-pcs/), [Euronews](https://www.euronews.com/my-europe/2026/05/04/eu-moves-to-ban-high-risk-inverters-from-china-over-cybersecurity-threats)
- Victron : « Feature request: MODBUS restriction » (flag « TCP MODBUS WRITES ALLOWED », interrupteur 3 positions) et question archivée « adding some security to the modbus » (0 réponse) — **dates non visibles, pages injoignables** — [Victron community](https://community.victronenergy.com/t/feature-request-modbus-restriction/31024), [archive](https://communityarchive.victronenergy.com/questions/128611/adding-some-security-to-the-modbus.html)
- evcc : 121 issues « Modbus » 2026, toutes des bugs de lecture/timeout (SMA, Huawei SUN2000, Fronius, AlphaESS), aucune sur authentification/audit — (API GitHub, 10 oct. 2026)

### Inferences
- Le besoin « audit Lituanie » n'est pas visible sur GitHub ; s'il existe, il se traite entre exploitants >100 kW et opérateurs réseau, hors forums open source.
- L'argument OASIS pour ce segment est « porte indépendante de l'EMS » (plage par registre, journal) plutôt que « conformité » : les fils montrent des écritures erronées ou involontaires, pas des attaques.

### Gaps
- Aucune source sur une obligation d'audit lituanienne ni sur une « restriction UE du 1er nov. 2026 » au sens technique ; la date correspond au grandfathering financier.
- SMA, Fronius, Huawei, SolarAssistant : pas de fil 2026 trouvé ; Victron non datable.

## Question clé 5 — CIRIS #402, MeshCore #1736, Reticulum, Meshtastic : autorisation de commande

### Takeaway
CIRIS #402 (2025, validation d'identité Discord) et MeshCore #1736 (backhaul Reticulum) sont hors sujet. Meshtastic a eu deux propositions 2026 d'autorisation signée des requêtes admin (#10971, #11247), toutes deux abandonnées (stale). Reticulum : aucun fil trouvé.

### Cited Findings
- CIRIS #402 : PR fusionnée 28 août 2025, « Removed username-based validation that allowed spoofing of Wise Authority users » — [CIRIS #402](https://github.com/CIRISAI/CIRISAgent/issues/402)
- MeshCore #1736 : discussion « Ideas » 18 févr. 2026, questions sur un backhaul Reticulum ; pramodhrachuri (10 mars) « The dev team is against tcp/ip bridging » ; dernier message 21 mai 2026 — [MeshCore #1736](https://github.com/ripplebiz/MeshCore/issues/1736)
- Meshtastic #10971 (10 juil.→13 sept. 2026, not planned) : « verified XEdDSA signature, and the signer key matches a populated admin_key slot » — [Meshtastic #10971](https://github.com/meshtastic/firmware/issues/10971)
- Meshtastic #11247 (27 juil.→23 sept. 2026, not planned) : mode `ADMIN_PKI`, réponse `NOT_AUTHORIZED` — [Meshtastic #11247](https://github.com/meshtastic/firmware/issues/11247)
- CVE-2025-55292 (janv. 2026) : identité Meshtastic liée au MAC, forge de NodeInfo, correctif 2.7.6 — [CERT.hr](https://cve.cert.hr/cve/CVE-2025-55292)

### Inferences
- Les projets mesh grand public n'ont ni client payant ni appétence pour une couche d'autorisation : valeur uniquement comme vitrine technique (rejeu après reboot).

### Gaps
- Reticulum (markqvist) : 0 résultat sur « command authorization replay signed ».

## Question clé 6 — IETF draft-morrison-ot-command-authority : implémenteurs, intérêt liste de diffusion 2026 ?

### Takeaway
Le draft existe et avance vite (-00 juil. 2026, -03 sept. 2026, 3 auteurs, soumission individuelle), accompagné d'un second draft complémentaire (draft-das-ot-actuation-finality-00, août 2026) qui vise explicitement Modbus. Aucun implémenteur ni fil de liste trouvé ; les pages IETF étaient injoignables et le budget de recherche épuisé avant une requête ciblée.

### Cited Findings ⚠️ (extraits de résultats de recherche uniquement)
- Morrison : « refuses a control action issued to an OT or industrial control system on a software agent's authority unless it carries verifiable proof of the agent's identity, the human principal it acts for, that principal's authorisation of the specific action, and an append-only audit record. It fails closed on authority, never on safety, and must not be placed in the trip path of a safety function. » ; -03 « lists three authors » ; datatracker : « active individual draft » — [datatracker](https://datatracker.ietf.org/doc/draft-morrison-ot-command-authority/), [-03](https://www.ietf.org/archive/id/draft-morrison-ot-command-authority-03.html)
- Das : « Modbus is often a raw register write with no act object at all », « a valid signed call can still be the wrong act », digest liant « device, tag, value, mode, and zone », autorité « single-use and tied to a specific sink » — [draft-das-ot-actuation-finality-00](https://datatracker.ietf.org/doc/html/draft-das-ot-actuation-finality-00)
- Liu : draft-liu-iotops-modbus-seriallink-sec-spec-07 (août 2026), sécurité des liaisons série Modbus, groupe IOTOPS — [txt](https://www.ietf.org/archive/id/draft-liu-iotops-modbus-seriallink-sec-spec-07.txt)

### Inferences
- Trois drafts actifs en 3 mois sur « autorité de commande OT / finalité d'actuation / sécurité Modbus série » = fenêtre pour qu'OASIS soit cité comme implémentation de référence ; cibler les auteurs et la liste IOTOPS.

### Gaps
- Noms/affiliations des auteurs, section Implementation Status, liste de diffusion : non lus (hôtes injoignables).

## Question clé 7 — OPC UA, DNP3 SAv6, IEC 62351 : qui veut un équivalent Modbus ?

### Takeaway
Rien de 2026 trouvé sur GitHub ou forums liant DNP3-SA/IEC 62351 à une demande d'équivalent Modbus ; le seul « équivalent Modbus » en cours est le travail SunSpec Secure Modbus (mTLS + RBAC) et les drafts IETF ci-dessus. Pas de source : traité comme gap.

### Cited Findings
- SunSpec Secure Modbus Test Procedures (mTLS, RBAC) en statut TEST, revue jusqu'au 1er août 2026 — [SunSpec](https://sunspec.org/events/sunspec-webinar-register-now-mid-year-standards-update/)
- Règlement Machines 1.1.9 (RISE) : composants « adequately protected against accidental or deliberate tampering », la machine doit « identify installed software and to record interventions in the software » ; application janv. 2027 — [RISE](https://www.ri.se/en/the-regulation-of-machinery-products-increases-focus-on-cybersecurity)
- NIS2 (ENISA, via un billet fournisseur) : distinction « execution of a signature » / « evidence supporting that execution », couche de preuve « tamper-evident, capable of surviving the compromise of the signing systems » — [ktsecure (fournisseur)](https://ktsecure.co.uk/insights/2026-05-28-the-evidence-architecture-gap-in-nis2-cryptographic-compliance)

### Inferences
- L'exigence 1.1.9 « record interventions » + NIS2 « evidence layer » donnent un angle réglementaire au journal chaîné OASIS, mais **aucun fil GitHub 2026 ne l'invoque** ; l'argument devra être porté par le vendeur, pas repris d'une demande client.

### Gaps
- Aucune discussion DNP3 SAv6 / IEC 62351 / OPC UA 2026 trouvée (recherche épuisée) ; aucun fil CRA/EN 50742 lié à Modbus sur GitHub.
