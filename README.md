# Plume

Vos cours de maths manuscrits, réécrits dans votre propre charte LaTeX.

Photographiez une page, importez-la : Plume la lit, la structure, et la rend
dans le modèle LaTeX que vous avez défini — prête à relire bloc par bloc, puis
à exporter en PDF.

## Prérequis

- **Claude Code**, installé et connecté à votre abonnement. C'est le moteur de
  lecture ; Plume n'utilise aucune clé d'API et ne parle à aucun service.
- Rien d'autre : le moteur LaTeX s'installe depuis les réglages, en un bouton.

## Développement

```bash
npm install
npm run desktop
```

`desktop` lance la vraie application — la fenêtre Tauri, avec l'accès aux
fichiers, à Claude Code et au moteur LaTeX. C'est ce qu'il faut dans presque
tous les cas.

| Commande | Ce qu'elle lance |
|---|---|
| `npm run desktop` | L'application, en développement |
| `npm run desktop:build` | Le paquet installable pour cette plateforme |
| `npm run dev` | L'interface seule dans le navigateur, sans backend |

`npm run dev` ne sert qu'au travail sur la mise en page : hors de la fenêtre
Tauri, aucune commande n'existe et l'application ne dépasse pas son écran
d'accueil. C'est aussi la commande que Tauri appelle lui-même
(`beforeDevCommand`), donc elle doit rester le serveur Vite : la renommer en
`tauri dev` ferait se relancer Tauri à l'infini.

```bash
npm run build                                  # tokens de style, tests, typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```

## Documentation

- [Reconnaissance, l'IR et la boucle de vérification](docs/recognition.md) — comment
  une photo devient du LaTeX, et ce que valent réellement les garde-fous.
- [Modèles](docs/templates.md) — clés, préambule, écriture des blocs, et pourquoi
  le modèle livré se duplique au lieu de s'éditer.
- [Empaquetage et publication](docs/packaging.md) — ce qui se construit où,
  signature, moteur LaTeX.
