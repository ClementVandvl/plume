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
npm run tauri dev
```

```bash
npm run build                                  # tokens de style, tests, typecheck
cargo test --manifest-path src-tauri/Cargo.toml
```

## Documentation

- [Reconnaissance, l'IR et la boucle de vérification](docs/recognition.md) — comment
  une photo devient du LaTeX, et ce que valent réellement les garde-fous.
- [Empaquetage et publication](docs/packaging.md) — ce qui se construit où,
  signature, moteur LaTeX.
