---
id: c365762b91011fe4a4ce
title: "Equazioni"
order: 25
created: 2026-10-18T00:00:00Z
updated: 2026-10-18T00:00:00Z
tags: []
---

NoteCraft disegna da sé la matematica in TeX, con veri caratteri matematici. Ogni formula di questa pagina è testo semplice: premi il pulsante `< >` per vederne il TeX.

## Nel testo, o da sola

Nel testo: $E = mc^2$, $\frac{a}{b}$ e $\sqrt{x^2+y^2}$ scorrono con le parole. Un blocco a sé è composto più grande:

$$
x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
$$

## Frazioni, radici, somme e integrali

$$
\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}
$$

$$
\int_0^{\infty} e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$

## Matrici, casi e allineamento

$$
\begin{pmatrix} a & b \\ c & d \end{pmatrix}^{-1} = \frac{1}{ad-bc}\begin{pmatrix} d & -b \\ -c & a \end{pmatrix}
$$

$$
|x| = \begin{cases} x & x \ge 0 \\ -x & x < 0 \end{cases}
$$

$$
\begin{aligned} (a+b)^2 &= a^2 + 2ab + b^2 \\ (a-b)^2 &= a^2 - 2ab + b^2 \end{aligned}
$$

## Accenti, alfabeti e simboli

$$
\hat{x} + \vec{v} + \bar{y} \qquad x \in \mathbb{R}, \; \mathcal{L}\{f\}(s) = \int_0^\infty f(t)e^{-st}\,dt
$$

$$
\nabla \cdot \mathbf{E} = \frac{\rho}{\varepsilon_0} \qquad i\hbar\frac{\partial}{\partial t}\Psi = \hat{H}\Psi
$$

## Parole dentro una formula

Le lettere che i caratteri matematici non hanno, come l'arabo e l'ebraico, sono disegnate con il carattere della pagina e si uniscono come devono:

$$
\text{المعادلة} \; x^2 = 4 \qquad \text{שלום} \; \pi r^2
$$

## Parti da un elenco

Inserisci, poi Equazione, apre un elenco di equazioni da cui partire. Scegline una e il suo TeX entra dove si trova il cursore.

Se una formula è sbagliata, NoteCraft ne mostra il testo invece di nasconderla.

Avanti: [Etichettare una riga](../tag-a-line/).
