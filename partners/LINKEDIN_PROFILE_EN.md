# OASIS : profil LinkedIn (anglais) et plan de prise de contact

Positionnement de référence : `partners/POSITIONING.md`. Tu colles toi-même ces
textes dans LinkedIn (Yadulink ne peut pas modifier un profil). Le ton est
technique, et rien n'est affirmé sans preuve dans `evidence/`.

---

## 1. Profil personnel

**Headline (220 car. max)**
> Building OASIS · No machine moves on a forged, replayed or revoked order · Command authority for autonomous fleets · Rust no_std on a $1 MCU · Looking for field pilots

**About**
> I build OASIS, the command-authority layer for autonomous machine fleets: drones, field robots, remote actuators.
>
> The question it answers: can this order move this machine? Fleet nodes sit in the open and get lost, stolen or opened. In most low-cost meshes one captured node can speak for all the others. Open meshes like Reticulum are built for anonymity, so their relays forward data without checking it.
>
> With OASIS, an order is executed only if it is signed by an authorised node, unmodified, fresh, not revoked, and if the machine's own sensors say it is safe to act. Every relay checks origin, content, freshness and network before forwarding. The operator can revoke a captured node fleet-wide without re-keying the others.
>
> Proven in the lab on three RP2040 boards (TRL 4):
> • 0 of 150 single-bit flips accepted, every packet traced
> • byte-exact replay refused after a real power-cut
> • revoked node dropped at the first hop, also after a power-cut
> • non-authorised, expired, replayed, lost-sensor and NaN orders never moved the actuator
> • Rust no_std, 646 tests, 173 Kani harnesses, proofs of the actuation rule
> • Not a certified safety function: no PL (ISO 13849-1), no SIL (IEC 62061). It reduces no machine risk — it decides whether an order is authentic, authorised, fresh and within limits.
>
> Not done yet: radio on hardware, routing, hardware key protection, external audit.
>
> I'm looking for 2–3 teams running drones, robots or remote actuators under one operator, for a 6-week field pilot on their hardware.

**Expérience (poste en cours)**
> Founder & lead engineer · OASIS · 2026 – present
> Command-authority layer for autonomous fleets, in Rust no_std. Signed envelopes verified at every relay, operator-signed fleet-wide revocation, seven-condition actuation gate with Kani proofs, MAVLink v2 bridge to PX4. Silicon test suite with raw logs and SHA-256 manifests.

**Compétences (5 à épingler)**
Rust · Embedded Systems · Applied Cryptography · Robotics · Drones

**Section « Sélection » (Featured)**
1. *OASIS Technical Brief* (page technique en anglais, déjà publique).
2. Le post « What failed on silicon » (§3) et son commentaire de correction.
3. Le post de positionnement (§3 bis), une fois publié.

## 2. Page entreprise OASIS (à créer à la main : LinkedIn → Pour les entreprises → Créer une page)

- **Nom :** OASIS
- **Slogan (120 car.) :** No machine moves on a forged, replayed or revoked order. Command authority for autonomous fleets.
- **Secteur :** Computer and Network Security
- **Taille :** 0-1 employé
- **Description :** le texte « About » ci-dessus, à la 3e personne.

## 3. Premier post (publié le 2026-10-06, avec un commentaire de correction)

> We ran our mesh security stack on three $1 microcontrollers. Here is what failed. […]

Texte complet et commentaire : voir l'historique Yadulink (post 6842). Ne pas republier.

## 3 bis. Post de positionnement (à publier, après validation)

> If one of your drones or robots is stolen tomorrow, what stops it from giving orders to the rest of the fleet?
>
> In most low-cost meshes, nothing: every node shares one key. In open meshes, relays forward data without checking it.
>
> I build OASIS, a command-authority layer for autonomous fleets. A machine acts only if the order is:
> → signed by a node allowed to command it
> → unmodified, checked at every relay
> → fresh, even across power cuts
> → from a node the operator has not revoked
> → and safe, according to the machine's own sensors
>
> Lab results on three $1 RP2040 boards: 0 of 150 tampered packets accepted, replays refused after a real power-cut, a revoked node dropped at the first hop, and no forged, expired or unsafe order ever moved the actuator.
>
> It is TRL 4: proven in the lab, not yet in the field. That is the next step, and I'm looking for 2–3 teams with drones, robots or remote actuators to do it with.
>
> #robotics #drones #embedded #rustlang #security

## 4. Les 10 profils cibles (révisés pour le nouveau positionnement)

| # | Persona | Mots-clés de recherche | Pourquoi |
|---|---|---|---|
| 1 | Responsable des opérations d'une flotte de drones | `drone fleet operations manager` | Le risque d'un drone capturé est concret |
| 2 | Ingénieur PX4 / autopilote | `PX4 engineer` | Point d'intégration du pilote (commandes MAVLink) |
| 3 | CTO d'une startup de robotique agricole | `CTO agricultural robotics` | Robots seuls sur des parcelles, risque de vol |
| 4 | Ingénieur sécurité OT / ICS | `OT security engineer` | Comprend la valeur d'un ordre authentifié sur un actionneur |
| 5 | Ingénieur automatisme sur sites isolés (eau, énergie) | `SCADA engineer remote sites` | Vannes et pompes commandées à distance |
| 6 | Responsable R&D robotique mobile | `mobile robotics R&D lead` | Flottes de robots sous un opérateur |
| 7 | Ingénieur sécurité offensive matériel / IoT | `hardware security engineer IoT` | Relecture critique du modèle de menace |
| 8 | Ingénieur Rust embarqué | `embedded Rust engineer` | Juge le code, peut relayer |
| 9 | Chef de programme dans un pôle drones, robotique ou industrie | `cluster program manager robotics` | Accès à plusieurs pilotes |
| 10 | Responsable innovation dans un incubateur deeptech | `deeptech incubator program manager` | Accès à plusieurs pilotes |

Les invitations déjà préparées et non envoyées (Reda Benmoulay, Elise Mondot,
Siméon Lecaux, Aymen Haggui, Saber Messadi, Igor P, et les profils au nom masqué)
restent pertinentes ; leurs notes sont mises à jour ci-dessous.

## 5. Notes d'invitation (≤ 300 caractères, sans lien, sans pitch)

**Opérations et intégration (drones, robots, sites isolés)**
> Hi {first_name}, I work on command authority for autonomous fleets: no machine acts on a forged, replayed or revoked order. If one of your machines were stolen, how would you exclude it from the fleet today? Curious how you handle it.

**Sécurité (OT, matériel, recherche)**
> Hi {first_name}, I built a Rust layer where every relay checks origin, content and freshness, and a machine acts only on authorised, safe orders. Proven on RP2040 silicon. Would value your critical take on the threat model.

**Écosystème (pôles, incubateurs)**
> Hi {first_name}, I'm building OASIS, command authority for drone and robot fleets, proven in the lab on silicon (TRL 4). Looking for field pilots to reach TRL 6. Could your network be a fit? Happy to share a short brief.

## 6. Règles d'envoi

- Au plus 10 invitations par jour au début : LinkedIn limite les comptes neufs.
- Pas de lien dans l'invitation. La page technique ne part qu'après acceptation, si la personne pose une question.
- Toujours dire « TRL 4, prouvé en labo » : ne jamais laisser croire à un produit déployé.
- Avant de répondre à « Is it open source? », décide du modèle de licence.
