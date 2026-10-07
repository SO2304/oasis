# Modbus TCP pour la passerelle (spec, première étape)

Origine : l'audit commercial du 2026-10-07. La plupart des machines récentes parlent
Ethernet (Modbus TCP, Profinet, EtherNet/IP), alors que la passerelle de la phase 1.4
ne parle que Modbus RTU. Sans TCP, aucun pilote n'est possible devant une machine
réelle. Cette étape ajoute le **transport TCP**. La **porte reste celle de la
phase 1.4** (`modbus_gateway::gateway_decision`, elle-même la porte de la partie F),
sans modification.

## 1. Architecture visée pour un pilote

```
IHM / SCADA ──Modbus TCP en clair──▶ agent OASIS ──ordre OMB1 signé (v0B)──▶ passerelle OASIS ──Modbus TCP──▶ automate
 (inchangée)                         (côté opérateur)                        (côté machine)                    (inchangé)
```

- **L'IHM n'est pas modifiée.** Elle écrit en Modbus TCP vers l'agent comme elle le
  ferait vers l'automate.
- L'**agent** lit l'écriture (`parse_tcp_write`). Il en fait un ordre `OMB1`
  (`TcpWrite::to_order`) avec son numéro de séquence, l'identifiant de démarrage
  (`boot_id`) de la passerelle et une échéance, puis le signe en v0B. Une fois la
  décision connue, il répond à l'IHM (`hmi_response`).
- La **passerelle** vérifie l'enveloppe v0B, décide (`decide_tcp`), et seule une
  décision `Act` produit une requête vers l'automate. Elle vérifie ensuite la
  réponse de l'automate (`check_tcp_response`).

## 2. Ce qui est fait (module `oasis-rt/src/modbus_tcp.rs`, pur, `no_std`)

| Élément | Règle |
|---|---|
| Requête vers l'automate | Construite à partir de la trame RTU d'une décision `Act` : même unité, même PDU, en-tête MBAP au lieu du CRC. **Aucune requête TCP sans `Act`.** |
| En-tête MBAP | `tid u16 | protocole 0 | longueur = 1 + PDU | unité`, big-endian |
| Fonctions | FC06 (1 registre) et FC16 (1 à 8 registres), comme en RTU |
| Écriture reçue de l'IHM | Longueur exacte, protocole 0, longueur MBAP cohérente, nombre d'octets FC16 = 2 × quantité, quantité 1..=8. Tout le reste est refusé (`None`), lectures comprises |
| Réponse de l'automate | `Ack` seulement si `tid`, protocole et unité correspondent, la longueur MBAP est cohérente, et l'écho FC06/FC16 est exact. Sinon `Exception(code)`, `Mismatch` ou `Short` |
| Réponse à l'IHM | Écho de l'écriture si elle a été exécutée et acquittée. Sinon une exception Modbus : 0x02 (registre hors carte), 0x03 (valeur hors plage), 0x0A (autre refus de la porte), l'exception de l'automate telle quelle, 0x0B (pas de réponse exploitable) |

Le code 0x0A pour un refus de politique est un choix : le protocole Modbus n'a pas de
code « non autorisé ». L'IHM voit un refus, pas une coupure silencieuse.

## 3. Preuves

- **7 tests `mbtcp_*`.**
  - Chaque requête construite est analysée et appliquée par **`rmodbus` 0.12.2 en
    mode TCP**, un serveur indépendant de notre code. La requête est identique,
    octet pour octet, à celle qu'une IHM enverrait.
  - Chaque condition de la porte bloque la requête.
  - L'analyseur refuse les trames mal formées.
  - La vérification des réponses et les réponses à l'IHM sont couvertes.
  - Un test de bout en bout chaîne IHM → agent → v0B → passerelle → automate
    `rmodbus`. L'automate n'est jamais touché par : une valeur hors plage, un
    registre hors carte, un rejeu octet pour octet, une origine forgée, une valeur
    modifiée dans une enveloppe authentique, ou une écriture Modbus brute emballée
    dans une enveloppe valide. Deux écritures valides = deux écritures sur
    l'automate.
- **2 harnais Kani** (`modbus_tcp/kani_proofs.rs`), vérifiés le 2026-10-07 avec
  cargo-kani 0.68.0 (CBMC 6.11.0), chacun avec une contre-épreuve qui échoue comme attendu :
  - `proof_mbtcp_frame_iff_act_and_matches_order` : une requête existe si et
    seulement si la décision est `Act`. Elle porte exactement l'ordre (MBAP,
    fonction, adresse de départ, valeurs).
    Contre-épreuve : `tid` altéré → FAILED.
  - `proof_mbtcp_parse_total` : l'analyseur ne panique sur aucune entrée jusqu'à
    37 octets, et ne rend que des écritures bornées.
    Première contre-épreuve sans effet : le harnais n'allait que jusqu'à 30 octets,
    donc n'explorait pas une FC16 à 9 registres. Harnais élargi, puis contre-épreuve
    (quantité 9 acceptée) → FAILED.

## 4. Ce qui n'est pas fait (prochaines étapes, `prompts/OASIS_PILOT_GATEWAY.md`)

1. **Les binaires réseau.** `oasis_mbtcp_agent` (écoute l'IHM) et
   `oasis_mbtcp_gateway` (écoute l'agent, parle à l'automate), sur Linux, avec
   délais d'attente, une seule connexion automate et une configuration de la carte
   des registres. Ici, tout se fait en mémoire, sans socket.
2. **Le temps de la passerelle.** L'agent doit connaître le `boot_id` et l'horloge de
   la passerelle. Le balisage OTM1 existe pour l'actionneur (partie F), mais n'est
   pas encore câblé en TCP.
3. **Le journal des interventions** exigé par 1.1.9 : chaque ordre accepté ou refusé,
   chaîné et signé.
4. **Les lectures** (FC03/FC04). Elles ne modifient pas l'état de la machine, mais une
   IHM en fait en permanence. Il faut décider : relais direct par l'agent, ou relais
   par la passerelle.
5. Un essai sur du matériel réel : un automate ou un simulateur Modbus TCP sur le
   réseau.
