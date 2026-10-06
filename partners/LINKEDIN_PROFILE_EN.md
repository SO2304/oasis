# OASIS : profil LinkedIn (anglais) et plan de prise de contact

Ce que tu colles toi-même dans LinkedIn : Yadulink ne permet pas de modifier un
profil ni de créer une page entreprise. Le ton est technique et non commercial,
et ne dit rien qui ne soit prouvé par `evidence/silicon/2026-10-06/`.

---

## 1. Profil personnel

**Headline (220 car. max)**
> Building OASIS · Signed multi-hop mesh for low-cost sensor nodes · Rust no_std, validated on RP2040 silicon · Looking for field pilot partners

**About**
> I build OASIS, a Rust no_std kernel that gives every node in a low-cost mesh its own signing identity.
>
> The problem: most cheap sensor meshes share one network key. Capture one node and you can impersonate every other node and replay their traffic. In OASIS, a captured node can only sign as itself, can be revoked, and cannot replay others' messages.
>
> What is validated today:
> • RP2040 (Cortex-M0+, 119 KB flash): RFC 8439 / RFC 7748 vectors, Ed25519, mesh forgery and replay rejection, 9/9 suite runs on 3 boards
> • A → B → C relay over a physical link with per-hop signature verification
> • Fault injection: 200 corrupted packets, no crash, every tampered signed field rejected
>
> What is not done yet: LoRa on hardware, power optimisation, key protection on RP2040, external audit.
>
> I'm looking for 2–3 teams deploying sensors or drones in the field for a 6-week pilot on their hardware.

**Expérience (poste en cours)**
> Founder & lead engineer · OASIS · 2026 – present
> Rust no_std mesh security kernel. ChaCha20-Poly1305 / X25519 / Ed25519, multi-hop mesh with per-node signatures, MAVLink v2 bridge to PX4. On-silicon test suite with archived raw logs and SHA-256 manifests.

**Compétences (5 à épingler)**
Rust · Embedded Systems · Applied Cryptography · Mesh Networking · LoRa

**Section « Sélection » (Featured)**
1. Lien vers la page technique en anglais (*OASIS Technical Brief*), à partager depuis le menu Share de la page.
2. Un post « What failed on silicon » (voir §3) : publier ses propres défauts est ce qui rend crédible auprès d'ingénieurs.

## 2. Page entreprise OASIS (à créer à la main : LinkedIn → Pour les entreprises → Créer une page)

- **Nom :** OASIS
- **Slogan (120 car.) :** Per-node signed mesh for low-cost sensor nodes. Rust no_std, validated on RP2040.
- **Secteur :** Computer and Network Security
- **Taille :** 0-1 employé
- **Description :** reprendre le texte « About » ci-dessus, rédigé à la 3e personne.

## 3. Premier post (avant toute prise de contact)

> We ran our mesh security stack on three $1 microcontrollers. Here is what failed.
>
> 1. One of our own tamper tests passed for the wrong reason: the packet was dropped as a duplicate, not by the MAC. Fixed, re-run on all boards.
> 2. Flipping single bits before the CRC: every flip in a signed field was rejected, but 12/50 flips in the payload were accepted. The mesh header is authenticated; payload integrity has to come from the AEAD layer.
> 3. An Ed25519 signature costs 174 ms on a Cortex-M0+ (we first reported 341 ms: that figure re-derived the key on every call). On this MCU, signing costs about as much energy as sending the packet over LoRa.
>
> What held: RFC vectors byte-exact on chip, 3-hop relay with per-hop verification, no crash under 200 corrupted packets.
>
> Next: LoRa on hardware, power measurements, RP2350 secure boot.
>
> #embedded #rustlang #iot #lora #security

## 4. Les 10 profils cibles

Recherche « 2e degré d'abord », profils en anglais ou en français, Europe. Il s'agit
d'un profil par persona : la recherche Yadulink sortira les personnes réelles, et
je te montrerai la liste avant tout envoi.

| # | Persona | Mots-clés de recherche | Pourquoi |
|---|---|---|---|
| 1 | Lead firmware dans une startup IoT agricole | `embedded firmware engineer LoRa agriculture` | Nœuds dispersés, sans surveillance, souvent volés |
| 2 | CTO d'une startup de surveillance de sites / périmètre | `CTO perimeter security sensors` | Un capteur capturé est une menace directe |
| 3 | Ingénieur sécurité IoT dans un industriel | `IoT security engineer OT` | Connaît le problème de la clé partagée |
| 4 | Architecte LoRaWAN / LPWAN | `LoRaWAN architect` | Peut dire si le mesh répond à un besoin que le réseau en étoile ne couvre pas |
| 5 | Ingénieur Rust embarqué (communauté Embassy / rp-rs) | `embedded Rust engineer` | Juge la qualité du code, peut relayer |
| 6 | Responsable R&D drones / essaims | `UAV swarm R&D engineer` | Face drone (PX4) |
| 7 | Chercheur en sécurité des réseaux de capteurs (labo, université) | `wireless sensor network security researcher` | Relecture critique, co-publication |
| 8 | Ingénieur dans un smart building / smart city | `smart city IoT engineer` | Grandes flottes de nœuds bon marché |
| 9 | Ingénieur hardware utilisant RP2040 / RP2350 en produit | `RP2040 product hardware engineer` | Cible matérielle exacte |
| 10 | Responsable innovation dans un pôle de compétitivité ou un incubateur deeptech | `deeptech incubator program manager` | Accès à plusieurs pilotes d'un coup |

## 5. Notes d'invitation (≤ 300 caractères, sans pitch)

**Ingénieur / CTO (personas 1, 2, 3, 8, 9)**
> Hi {first_name}, I work on per-node signed mesh for cheap sensor nodes (Rust, tested on RP2040). Curious how you handle a stolen or opened node in your deployments today. Happy to share my silicon test results if useful.

**Profil LoRa / Rust (personas 4, 5)**
> Hi {first_name}, I'm testing Ed25519 per-node signatures on a Cortex-M0+: 174 ms per sign, about the energy of a LoRa TX. Would value your take on where that trade-off makes sense.

**Recherche / drones / écosystème (personas 6, 7, 10)**
> Hi {first_name}, I built a signed multi-hop mesh kernel in Rust, validated on 3 RP2040 boards with raw logs. Looking for critical feedback and field pilots. Open to a 15-min exchange?

## 6. Règles d'envoi

- Pas plus de 10 invitations par jour au début : LinkedIn limite les comptes neufs.
- Ne jamais joindre de lien dans l'invitation. Le lien vers la page technique ne part qu'après acceptation, et seulement si la personne pose une question.
- Avant de répondre « oui » à un « Is it open source? », décide du modèle de licence (voir le §4 de la discussion sur le modèle économique).
