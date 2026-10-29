# Druk Language: Future Features & Roadmap

Druk is designed to be a **Simple, High-Performance, GC-backed** language. We want developers to focus on logic without worrying about manual memory management, while still enjoying the speed of an LLVM-compiled binary.

En s'inspirant des géants modernes comme **Python** (pour l'ergonomie, la productivité et l'écosystème) et **Rust** (pour la sécurité, l'outillage et l'expressivité), voici la liste priorisée des fonctionnalités critiques (Gigantic Features) qui manquent encore à Druk pour devenir un langage de production incontournable.

---

## 💎 Priorité 1 : Ergonomie & Expressivité (Inspiré de Python & Swift)

### 1. String Interpolation (f-strings)
*   **Feature**: Syntax `f"Bonjour {nom}, tu as {calcul()}"`.

### 2. Décorateurs (Meta-Programming)
*   **Feature**: Syntax `@cache`, `@benchmark`, ou `@auth` au-dessus des fonctions.
*   **Pourquoi c'est vital (Python)**: Permet d'ajouter du comportement sans modifier la logique métier. Druk peut les implémenter via des hooks au moment de la génération d'IR.

### 3. List Comprehensions & Générateurs
*   **Feature**: `[x * 2 for x in ma_liste if x > 0]` et le mot-clé `yield`.
*   **Pourquoi c'est vital (Python)**: Manipulation de données extrêmement concise et efficace (paresseuse).

### 4. Named & Default Parameters
*   **Feature**: `ma_func(nom="Druk", version=1.0)`.

---

## 🛡️ Priorité 2 : Sécurité Typage & Contrôle (Inspiré de Rust & TypeScript)

### 1. Null Safety Absolue (Types Optionnels)
*   **Feature**: Type `?T` (ou `Option<T>`). Pas de `null` par défaut.

### 2. Gestion des Erreurs Error-as-Value (Result<T, E>)
*   **Feature**: Pattern `try/catch` dzongkha-isé ou retour de `Result`.

### 3. Traits / Protocols
*   **Feature**: Définition de contrats de types sans héritage rigide.

### 4. Union Types & Exhaustive Matching
*   **Feature**: `type Status = Success | Loading | Error`. Le compilateur vérifie que `match` couvre tous les cas.

### 5. Const-Correctness Globale
*   **Feature**: Mot-clé `གཏན་འཇགས་` (const) pour garantir l'immutabilité par défaut.

---

## 🚀 Priorité 3 : Concurrence & Performance (Inspiré de Go & Mojo)

### 1. Concurrence Légère (Goroutines Druk)
*   **Feature**: Mot-clé `འགྲུལ་` (spawn) pour des threads légers.

### 2. Channels (CSP)
*   **Feature**: Communication sécurisée entre tâches via `chan`.

### 3. FFI Statique & Zéro-Copie
*   **Feature**: Appeler du C/C++ sans overhead de performance (parfait pour le ML).

### 4. Compile-Time Evaluation (comptime)
*   **Feature**: Exécuter du code Druk pendant la compilation pour générer des constantes ou du code optimisé (Inspiré de Zig).

---

## ⚙️ Priorité 4 : Écosystème & Outillage (Inspiré de Cargo & NPM)

### 1. Unified CLI (`druk`)
*   **Feature**: `druk build`, `druk test`, `druk doc`, `druk fmt`.

### 2. Package Registry (DrukCentral)
*   **Feature**: Gestionnaire de dépendances intégré avec lockfile.

### 3. Hot Reloading (REPL Avancé)
*   **Feature**: Modifier le code et voir les changements en direct dans le JIT.

### 4. Language Server Protocol (LSP)
*   **Feature**: Support autocomplétion, renommage, et diagnostics en temps réel dans VS Code.

---

## 🌍 Priorité 5 : Dzongkha-Native Ecosystem

### 1. Standard Library "Batteries Included"
*   **Feature**: `druk.net`, `druk.json`, `druk.fs`, `druk.ui` (via bindings Skia ou Qt).

### 2. Documentation Générée Nativement
*   **Feature**: Les commentaires (si on en autorisait un jour...) extraits automatiquement en PDF/HTML avec typage.

---

## 🔬 Priorité 6 : Métaprogrammation & Réflexion Statique

### 1. Gabarits (Generics) de Haute Performance
*   **Feature**: `type Liste<T> = ...`.
*   **The Goal**: Réutiliser le code sans perdre 1 seul cycle CPU grâce à la monomorphisation (comme en Rust/C++).

### 2. Réflexion à la Compilation (Introspection)
*   **Feature**: Pouvoir itérer sur les champs d'une struct ou les méthodes d'une classe *pendant* la compilation.
*   **Pourquoi c'est vital (Zig/Mojo)**: Permet de générer automatiquement du code pour la sérialisation (JSON, Protobuf) sans aucun outil externe.

### 3. Macros de Procédure (Hygiéniques)
*   **Feature**: Des plugins de compilateur écrits en Druk qui transforment l'AST avant la génération d'IR.

---

## 💻 Priorité 7 : Systèmes Critiques & Bas Niveau (Inspiré de Mojo/C++)

### 1. Gestion de Mémoire Hybride (Manual + GC)
*   **Feature**: Possibilité d'allouer manuellement dans des `Arenas` ou sur la `Stack` pour des performances extrêmes, tout en gardant le GC pour le reste.
*   **The Goal**: Donner au développeur le contrôle total quand il en a besoin (ex: moteurs de jeux, drivers).

### 2. Registres SIMD et Intrinsics
*   **Feature**: Accès direct aux instructions AVX-512 / ARM Neon via des types intrinsèques.

### 3. Inline Assembly (འཕྲུལ་ཆས་)
*   **Feature**: Insérer de l'assembleur natif ou du LLVM IR directement dans une fonction Druk.

---

## 🌐 Priorité 8 : Architectures, Distribution & Hardware

### 1. WebAssembly (Wasm) Natif
*   **Feature**: Compiler Druk pour le Web et pour WASI (server-side Wasm).
*   **The Goal**: Exécuter du Druk dans le navigateur à une vitesse proche du fer.

### 2. GPU Kernels (SIMT)
*   **Feature**: Écrire des fonctions qui compilent directement en kernels CUDA ou Metal.
*   **Pourquoi c'est vital**: Permettre au Druk de devenir un langage de choix pour le calcul scientifique et l'IA.

### 3. Cross-Compilation "Zero-Config"
*   **Feature**: `druk build --target aarch64-apple-darwin` depuis Windows, sans installer de toolchain complexe.

---

## 🤖 Priorité 9 : Intelligence Artificielle & Data Science

### 1. Tenseurs Natifs (First-Class Tensors)
*   **Feature**: Le type `Tenseur` est intégré au langage, avec support du slicing à la NumPy.

### 2. Différenciation Automatique (Autograd)
*   **Feature**: Le compilateur peut dériver automatiquement les fonctions pour le Gradient Descent.

### 3. Python-Interop Fluide
*   **Feature**: Importer des librairies Python (PyTorch, Pandas) directement dans Druk avec zéro overhead.

---

## 🔒 Priorité 10 : Sécurité Avancée & Vérification Formelle

### 1. Vérification Formelle Statique
*   **Feature**: Prouver mathématiquement que certaines fonctions ne paniquent jamais ou ne causent jamais de débordement d'entier.
*   **The Goal**: Sécurité absolue pour les systèmes critiques (finance, médical, aérospatial).

### 2. Taint Analysis (Sécurité des Données)
*   **Feature**: Le compilateur suit les données utilisateur non filtrées et empêche leur utilisation dans des fonctions sensibles (ex: SQL, Shell) sans assainissement.

---

## ⌛ Priorité 11 : Time-Travel Debugging & State Management

### 1. Time-Travel Debugging Natif
*   **Feature**: Le runtime peut enregistrer l'exécution et permettre de "reculer" dans le temps pendant le debug pour trouver la source d'un bug.

### 2. Snapshots de Heap (Persistance Transparente)
*   **Feature**: Sauvegarder l'état complet de la mémoire du programme sur disque et le restaurer instantanément plus tard ou sur une autre machine.

---

## ☁️ Priorité 12 : Cloud & Distributed Computing

### 1. Transparent RPC (Remote Procedure Calls)
*   **Feature**: Appeler une fonction sur un autre serveur comme si elle était locale. Le compilateur génère tout le code de sérialisation et de réseau automatiquement.

### 2. Distributed Garbage Collection
*   **Feature**: Un GC qui gère les références d'objets à travers plusieurs machines dans un cluster.

---

## 🎮 Priorité 13 : Moteur de Jeu & Graphismes (Druk Engine)

### 1. Shaders en Druk Natif
*   **Feature**: Écrire ses shaders GLSL/HLSL directement en syntaxe Druk. Le compilateur s'occupe de la compilation vers SPIR-V.

### 2. Hot-Reloading de Scène
*   **Feature**: Modifier la logique d'un jeu ou les paramètres d'un rendu sans jamais redémarrer le processus.

---

## 📏 Priorité 14 : Unité & Mesures (First-Class Units)

### 1. Types de Mesures Statiques
*   **Feature**: `གྲངས་ distance = 10m; གྲངས་ temps = 2s; གྲངས་ vitesse = distance / temps;`. Le compilateur vérifie les unités (interdire d'ajouter des mètres et des secondes).
