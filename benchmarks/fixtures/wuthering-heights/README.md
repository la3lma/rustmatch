# Wuthering Heights development fixture

These files provide rustmatch's stable, realistic development and coarse CI
regression corpus.

- `wuthr10.txt` is Project Gutenberg Etext #768, *Wuthering Heights* by Emily
  Bronte. Its complete historical Project Gutenberg header and distribution
  terms remain in the file.
- `real-words-in-wuthering-heights.txt` is the deterministic word list used by
  the original Java rmatch development benchmarks.
- Both files were copied from `la3lma/rmatch` at commit
  `b59894d73e90caf917bc723bce73ac2012ab6f35`.

SHA-256:

```text
44ea6e83ae3cd1ee15262e07344611910ecfd7dbfd6524b977e9f0f5af9cf260  wuthr10.txt
103cd748390b7407ab75284ef6f72c334b4e78d7ae9148e11aa0e8a23287895b  real-words-in-wuthering-heights.txt
```

The fixture is intentionally retained even after larger and more diverse
corpora are added. It provides a continuous historical line; it is not by
itself evidence that performance generalizes to every language or workload.
