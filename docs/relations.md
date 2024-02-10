# Graphs all the way up (and down)

Consider the set of all possible states $ \mathbb{S} $ for our system.  At each point in time, our system is in one of those states $ S \in \mathbb{S} $.  To evolve the system from one state to another, we have a protocol.

The protocol describes which transitions are possible.  Formally, we can identify the protocol as a directed graph $ G = (\mathbb{S}, E) $ where $ E \subseteq \mathbb{S} \times \mathbb{S} $.

(Of course, we never have access to the entire state space $ \mathbb{S} $, so we can't actually construct the graph $ G $.  But we can still reason about it.)

Our state can be thought of as a product of simpler states.  That is, we can write $ S = \prod_{o\in \mathbb{O}} s_o $, where $ \mathbb{O} $ is the set of all possible observables and $ s_o $ is the state of observable $ o $.  The set of all possible states for observable $ o $ is $ \mathbb{S}_o $.  Thus $ \mathbb{S} = \prod_{o\in \mathbb{O}} \mathbb{S}_o $.  We will occasionally use $o: \mathbb{S} \rightarrow \mathbb{S}_o$ as a function that extracts the state of observable $o$ from a state of the entire system.

An observable can be a (named) object or a (named) global context variable, like block height or time.  For sake of simpliicity, we treat the set of observables as fixed and known in advance.  But that just means that before an object is *created* or after it is *destroyed*, we can pretend it has an implicit null state.

Each observable defines its own protocol, which is a directed graph $ G_o = (\mathbb{S}, E_o) $ where $ E_o \subseteq \mathbb{S} \times \mathbb{S} $.  The protocol for the entire system is the intersection of the protocols for each observable: $ G = \bigcap_{o\in \mathbb{O}} G_o $, specifically $ E = \bigcap_{o\in \mathbb{O}} E_o $, or equivalently $ E \subseteq E_o $ for all $ o\in \mathbb{O} $.

(Because all of our graphs have the same set of vertices, we will usually omit the vertex set and just write $ E $ for the protocol.)

Note how the protocol for the entire system is the intersection of the protocols for each observable.  In other words, **each constituent protocol has a veto** over which state transitions are allowed.  This is a very powerful idea, and it's the key to our approach.

To get a useful protocol out of this, we require that each observable's protocol allow all state changes that leave its own observable unchanged.  Formally, we require that $E_o \supseteq \operatorname{const}_o$ where $ \operatorname{const}_o := \left\{ (s, t) \mid o\left(s\right) = o\left(t\right), s, t \in \mathbb{S}\right\} $.  We will also occasionally refer to this protocol as $ o = o $ or as $ =_o $.

<!-- ---

> Everything below here is just random notes.

---

The set of states $ \mathbb{S} $ is a poset, with the partial order $ \leq $ defined by the protocol.  That is, $ S \leq T $ if and only if there is a path from $ S $ to $ T $ in the graph $ G $.

---

TODO: figure out how to talk about cross program calls?  (And witnesses.  The witness is the important idea, because it's often what restricts allowed transitions in practice.  Especially for a wallet.  Perhaps put that in as an extension at the end?  It's also related to constructivism?) -->
