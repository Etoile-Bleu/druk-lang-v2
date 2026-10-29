# Documentation Officielle du Langage Druk

Druk est un langage de programmation typé et compilé (JIT/AOT via LLVM) qui utilise la syntaxe et les mots-clés en Dzongkha (langue officielle du Bhoutan). Ce document explique les différentes fonctionnalités du langage accompagnées d'exemples concrets.

## 1. Types de base et Variables

Les variables doivent indiquer leur type lors de leur déclaration.

Mots-clés des types :
- `གྲངས་` : Nombre (Entier)
- `ཡིག་འབྲུ་` : Chaîne de caractères (String)
- `བདེན་རྫུན་` : Booléen
- `སྟོང་པ` : Vide (Void, utilisé principalement pour indiquer l'absence de retour d'une fonction)

Exemples :
```druk
// Nombres
གྲངས་ age = 30;

// Chaînes de caractères
ཡིག་འབྲུ་ nom = "Tashi";

// Booléens (vrai = བདེན་པ་, faux = རྫུན་མ་)
བདེན་རྫུན་ est_majeur = བདེན་པ་;
```

## 2. Affichage Console (Print)

Pour afficher un résultat dans la console, utilisez le mot-clé `བཀོད་` (print).

```druk
བཀོད་ "Bonjour le monde !";
བཀོད་ 42;
```

## 3. Interpolation de Chaînes de Caractères

L'interpolation de chaînes de caractères permet d'insérer directement des variables ou des expressions à l'intérieur d'une chaîne en utilisant les accolades `{}`.

```druk
ཡིག་འབྲུ་ nom = "Tashi";
གྲངས་ age = 30;

བཀོད་ "Bonjour {nom}, tu as {age} ans !";

// Vous pouvez même y intégrer des expressions mathématiques :
བཀོད་ "Dans 10 ans, tu auras {age + 10} ans.";
```

## 4. Structures de Contrôle (Conditions)

Le langage supporte les blocs conditionnels standard `if` / `else`.

Mots-clés associés :
- `གལ་སྲིད་` : Si (If)
- `མེད་ན་` : Sinon (Else)

```druk
གྲངས་ score = 85;

གལ་སྲིད་ (score >= 90) {
    བཀོད་ "Excellent !";
} 
མེད་ན་ གལ་སྲིད་ (score >= 50) {
    བཀོད་ "Passable.";
} 
མེད་ན་ {
    བཀོད་ "Échec.";
}
```

## 5. Pattern Matching (Filtrage par motif)

Druk propose un système de filtrage par motif puissant, initié par le mot-clé `འགྲིག་པ་` (match). Le cas par défaut s'écrit avec la cible `_`.

```druk
གྲངས་ jour = 3;

འགྲིག་པ་ jour {
    1 -> བཀོད་ "Lundi";
    2 -> བཀོད་ "Mardi";
    3 -> བཀོད་ "Mercredi";
    _ -> བཀོད་ "Un autre jour";
}
```

## 6. Les Boucles

Druk supporte plusieurs types de boucles pour gérer l'itération.

### Boucle Infinie (`རིམ་པ་`)
```druk
གྲངས་ compteur = 0;

// Attention, implémentez une logique de sortie ou une fonction `return`
རིམ་པ་ {
    བཀོད་ compteur;
    compteur = compteur + 1;
}
```

### Boucle Tant Que (`ཡང་བསྐྱར་`)
```druk
གྲངས་ n = 0;
ཡང་བསྐྱར་ (n < 3) {
    བཀོད་ "Itération while !";
    n = n + 1;
}
```

### Boucle Pour (`རེ་རེར་`) - Format classique
```druk
རེ་རེར་ (གྲངས་ i = 0; i < 5; i = i + 1) {
    བཀོད་ i;
}
```

## 7. Les Tableaux

Vous pouvez créer et utiliser des tableaux en déclarant explicitement un type de base suivi par des crochets `[]`.

```druk
// Un tableau de nombres
གྲངས་[] nombres = [10, 20, 30];

// Accès à un élément par son index (indexé à partir de 0)
བཀོད་ nombres[0]; // Affiche 10
```

## 8. Les Fonctions et la Récursivité

Les fonctions se déclarent avec le mot-clé `ལས་འགན་` (Function). 
Le mot-clé pour retourner une valeur est `སླར་ལོག་` (Return).

Le type de retour est spécifié après les paramètres en utilisant une flèche `->`. Si aucune flèche n'est précisée, le retour est considéré comme `སྟོང་པ` (Void).

```druk
// Déclaration de fonction classique
ལས་འགན་ addition(གྲངས་ a, གྲངས་ b) -> གྲངས་ {
    སླར་ལོག་ a + b;
}

གྲངས་ resultat = addition(10, 5);
བཀོད་ resultat; // Affiche 15
```

### Récursivité
Druk supporte la récursivité pour l'appel de fonctions sur elles-mêmes.
```druk
ལས་འགན་ factorielle(གྲངས་ n) -> གྲངས་ {
    གལ་སྲིད་ (n <= 1) {
        སླར་ལོག་ 1;
    }
    སླར་ལོག་ n * factorielle(n - 1);
}

བཀོད་ factorielle(5); // Affiche 120
```

## 9. Les Fonctions Anonymes (Lambdas) et Cadrage Fonctionnel

Les fonctions sont des entités de "première classe" en Druk (ce qu'on appelle la programmation fonctionnelle). Elles peuvent être assignées à des variables, ou passées en argument comme des données traditionnelles. 

Les lambdas (fonctions anonymes) sont créées très rapidement avec l'opérateur flèche `->`.

```druk
// Lambda à plusieurs arguments
(གྲངས་, གྲངས་) -> གྲངས་ op = (x, y) -> x * y;
བཀོད་ op(5, 5); // Affiche 25

// Lambda à un argument
གྲངས་ carre = n -> n * n;
```

#### Passage d'une fonction en paramètre d'une autre (Ordre Supérieur) :
Vous pouvez créer de puissantes fonctions génériques qui englobent d'autres manipulations :

```druk
ལས་འགན་ generique(གྲངས་ argument, operateur) {
    སླར་ལོག་ operateur(argument);
}

// Appel de la méthode
བཀོད་ generique(5, n -> n * n); // Affiche 25
```
