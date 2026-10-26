# How to write here

This guide governs everything in `docs/` and every code comment. Write for a competent technical reader who has not worked on Mosaica, such as an assessor checking the security argument, an engineer evaluating the approach or a contributor arriving cold, because each of them needs to follow the text without help from someone who already knows the system.

## Rules

1. **Describe the system, not its history.** Say what the system is and why it is that way. The reader does not need to know which revision changed it, who found a problem in review or what it used to be, because that history lives in git. For the same reason, do not use phase or task numbers, "currently" or "for now".

   > ✗ "r3 replaced r1's retirement stamp, because for suppressions that was fail-open."
   >
   > ✓ "A suppression is removed only when it is lifted, because a retirement stamp would let it expire and re-expose the item."

2. **Mark what is not built, where the claim is made.** Present tense about machinery that does not exist reads as an assurance, so write "Not built yet:" followed by what happens instead and whether that is safe. Put the note beside the claim it qualifies, since a reader who skips the preamble will otherwise miss it.

3. **Make each paragraph stand alone.** Expand an acronym or project term the first time it appears in each document. When you refer the reader elsewhere, say what they will find there ("the rejected alternative is in §7.2"), and do not let the reference stand in for the text the reader needs here.

4. **Cover the substance and stop.** A document does not need an introduction that restates its title, a summary that restates its body or a discussion of alternatives nobody proposed. If a reader cannot follow a paragraph at reading speed on the first pass, it is either padded or compressed.

5. **Use emphasis rarely.** Bold is for a defined term where it is defined, or for the head of a list item. Do not call a design choice novel or key, and give a mechanism the space its consequences warrant rather than the space it took to get right.

6. **Draw structures, flows and state machines.** Use Mermaid in a fenced block with a caption. A diagram that repeats a paragraph is padding, but one that replaces it is the right choice. A comparison of three or more things is clearer as a table.

7. **Say what is true, precisely.** Distinguish what was measured from what was modelled and what was assumed, and say which applies. Record negative results ("F3: not confirmed by measurement"), and say what something does not do when its scope could be disputed. Argue rather than assert, so that a reader who disagrees can find the reason and test it.

8. **Use British spelling** and the established security vocabulary, such as conservative label join, boolean expression indexing, partial evaluation, the Non-Truman model and compartmented MAC.

## Register

Write plain technical English in complete sentences, each with a subject and a verb.

**Connect ideas rather than listing them.** This matters more than any other rule in this section. Each sentence should make one point, and any other clause in it should serve that point by saying when, why, how or with what result. Say how ideas relate with words such as *because*, *when*, *so*, *which means* and *is used for*. Do not set facts side by side and leave the reader to connect them, whether they are separated by commas, semicolons or full stops, because splitting a list of facts into short sentences does not connect them. Do not add facts to the end of a sentence as trailing phrases (", with…", ", such as…", ", which…") when they are not part of its point. Keep the small words that make a sentence read as spoken, and make a text shorter by saying less, not by removing the words that hold it together.

> ✗ A card showing the picked item. Lists its fields, with Open and Copy id. Shows the views it appears in.
>
> ✓ The item card shows the record of the item a viewer has picked on the map, so that they can read its fields and open it elsewhere.

The following habits also make text harder to read, so do not write them.

- **Aphorisms or slogans in place of rules.** Write the rule itself, as in "The service does not refuse a deny because it is busy."
- **Antithesis.** Phrases such as "not X, but Y", "X, not Y" and "this is not X; it is Y" make the reader hold a wrong idea before the right one, so say what the thing is.
- **Metaphor as jargon.** Words such as load-bearing, discharges, obligation, owes, the lane, the seam, the frontier, the gate, the fold, the spine, rides and machinery hide the technical term, so use the technical term instead. "Fail-open" and "fail-closed" describe access decisions and nothing else.
- **Abstract nouns for actions.** Write "when the entry is removed" rather than "the retirement position of the entry".
- **Em-dashes.** A well-built sentence does not need one, so rewrite a sentence that seems to.
- **Justifying a rule by its importance or by the failure it prevents.** Phrases such as "which is the point", "by construction", "deliberately", "importantly", "note that" and "the failure this exists to prevent" add emphasis without information. Give the reason once, as part of the sentence that states the rule.
- **Anecdotes as justification.** A story such as "caught in review twice" or "shipped three defects" is history, so keep the reason it illustrates and drop the story.
- **Symmetry for effect.** Paired clauses, triads and mirrored sentences draw attention to their shape rather than their content.
- **Absolutes and drama.** Do not write "silently", "quietly", "catastrophic", "never" or "always" unless the claim really is absolute.
- **Hedging.** Words such as "arguably", "in practice", "generally" and "tends to" leave the reader unsure what is true, so say what is true or leave the point out.
- **Summaries that begin with a pronoun.** Name the subject, because "this" and "it" at the start of a summary often have no clear antecedent.
- **Instructions disguised as observations about the reader.** If the reader should do something, tell them to do it.

## Code comments

Code comments follow the same rules. A module's doc comment carries the design argument when the module upholds an invariant the code does not show, rejects an obvious construction for a reason that is not obvious, or has a shape that a measurement decided. Otherwise it needs only a few lines on what the module is for. State a rule once, in the place where it belongs, and refer to it from elsewhere. Do not leave `TODO` or `FIXME` comments, because open work belongs in an issue.

## User documentation

The rules above were written for the system chapters, whose reader is assessing a design. The pages in `docs/start/`, `docs/guides/` and `docs/reference/` have a different reader, who is learning Mosaica or using it to get something done, so where this section differs from the rules above, this section applies on those pages. British spelling and the evidence rules apply everywhere.

There are three kinds of user page, and each is written differently because it serves a different moment. A tutorial teaches a newcomer by walking them through building something, a guide helps someone who knows the basics to do one task, and a reference page is somewhere a reader looks something up.

### Tutorials

A tutorial should read as if a knowledgeable, patient person were sitting beside the reader while they do something for the first time.

It opens with a promise. Say who the tutorial is for, what the reader will have built by the end, what they will understand that they did not before, roughly how long it takes and what they need before starting. Then show the journey before walking it, with a short outline of the stages, so that every later step has somewhere to hang and the reader always knows where they are.

A tutorial teaches a mental model rather than a list of commands. Before each step, say what we are about to do and why it is needed, and after it, say what happened and what to notice in the output. By the end the reader should be able to explain the whole pipeline to someone else: the source data, a declaration that describes it, a build that turns both into a bundle, a server with three separate listeners, and the tokens that decide what each viewer sees.

Introduce each idea at the moment it is needed, one at a time, giving the concrete case first and the name second, in plain words: "Every place on this map is tagged with who may see it. Mosaica calls that tag an access label." A term the reader has not met is never used without an introduction like this. Deeper explanation can wait for a link to the system chapters, but the basic answer to "what is this?" cannot.

When you explain a setting, say what it is for, what the alternatives are and what each one costs, because the reader will face the same choice with their own data. `render`, `index` and an attribute with neither are one decision, and a tutorial that shows only the value it uses leaves the reader unable to make that decision.

Frame Mosaica in terms the reader already knows. It is a database built to stream millions of points to a browser, so where the difference between it and the databases they have used explains a step, say what that difference is.

Leave out anything that does not help the reader do the task or understand it, such as timings, version lists, counts that only decorate a sentence, asides and hints about what comes later. The evidence rules keep every figure true, but they are not a reason to include one.

Show the reader their progress early and often. When a `check` comes back clean, a count matches what was expected or a map appears, point it out.

Never let the reader get lost. Every step must work exactly as written, and the expected output is shown so that they can tell they are on track. Where something is likely to go wrong, say what that will look like and what to do about it. Follow one path through the task, because choices between paths belong in guides.

Invite a little exploration where it teaches something, for example by removing a required key to watch the check fail, or by applying a second filter to watch the count change.

End by consolidating what the reader built and the ideas they now understand, and point them to one or two natural next steps.

The voice is warm, confident and conversational. "We" is natural for work done together and "you" for what the reader does. When something looks odd, admit it and explain it. Personality comes from specifics and from having a view, and enthusiasm or exclamation marks do not supply it.

### Guides

A guide opens with the task and with when you would need to do it, in a few sentences. Each step says briefly why it matters and then how to do it. Terms are defined where they first appear, or linked to the place that defines them. Warmth is welcome, but the reader has come to finish a job, so a guide explains less than a tutorial and moves faster.

### Reference

A reference page is looked up rather than read, so it is complete and has no narrative. It covers every key, flag, route, type or method, with its type, its default, what it does and what it refuses. Each entry starts with what kind of thing the item is and what it is used for, so that a reader scanning the page can tell at once whether this is the entry they want. Detail that only some readers need comes after that, or on the member it belongs to.

> ✗ `<mosaica-colour-editor>`: Every value or cluster's colour, to search, choose and reset.
>
> ✓ `<mosaica-colour-editor>`: A dialog for choosing the colour of each category value or cluster.

### Evidence

Every command on a user page has been run, and its output is pasted exactly as it was printed. Shorten long output with a line reading `...` rather than retyping any of it, and keep the numbers the run printed even when they are untidy. When a page is revised, run its commands again.

Examples use real data and real names, such as the GeoNames extract the tutorial builds, its `feature_class` column and the refusal the server actually returns. Do not use `foo`, `my_column` or `example.com` when a real value exists.

When you mention an error, quote its message exactly and say what to change to fix it.

### What makes prose read as generated

Structure on its own does not make prose read as generated, since an introduction, an outline and a closing summary all belong in a tutorial. What gives them away is being generic, as with a promise of "a powerful, interactive visualisation", an outline that repeats the headings or a summary the reader could have guessed before starting. Fill each of them with specifics that only this tutorial could contain.

Most of the other signs are in the sentences, and the commonest is the one described under **Connect ideas rather than listing them** above. The rest are these.

- **Compound sentences, one after another.** When sentence after sentence joins two clauses with ", and", ", so", ", but", a semicolon or a colon, the joins stop meaning anything. Join two clauses when they make one thought, and not to avoid a full stop.
- **Runs of short sentences in the same shape.** "It keeps shapes true. It stretches areas. It has no default." Four sentences in a row that begin "You'll" read as mechanically as a chain of clauses does. Write as a person talks, in mostly plain sentences of varied length, with a longer one where an idea needs carrying through and a short one where it lands.
- **Symmetry.** Paragraphs of the same length, lists whose items all take the same shape and parallel clauses balanced against each other all make a page feel assembled. Mix sentence lengths and styles, so that a statement may be followed by the question a reader would ask, and let one paragraph run long while the next is a single line. A page with some unevenness reads as if someone wrote it.
- **A colon followed by a list of three.** In "One program does all the work: it checks, builds and serves", the colon announces a list instead of saying anything. Say one thing, or show the three as they happen.
- **Sentences about the text instead of the subject.** "There are six stages, and each one leaves something you can look at" and "This is the step that decides what a viewer sees" talk about the page. Cut them, or replace them with the fact they gesture at.
- **Framing that nobody would say aloud.** Phrases such as "the pieces every deployment is made of" and "the whole path from a text file to a viewer's map" stand in for the things themselves, so name the actual things.
- **Metaphor for a technical part.** Calling parts doors, lanes or gates hides what they are, so use the real name and explain it in plain words.
- **Claims no run could contradict.** "Mosaica handles large data efficiently" tells the reader nothing, so give the figure and the conditions it was measured under, or cut the claim.
- **Enthusiasm, hype and reassurance in place of information.** "That's it!", "don't worry" and "seamlessly" take the place of something the reader could use.
- **Explanations that restate the command.** Say what the command does and why, rather than repeating it in words.
- **Three examples where one would do,** or options set side by side only for balance.

`scripts/check-register.sh` rejects the stock phrases of generated documentation on user pages, and the script holds the list. It cannot see how sentences are built, so review has to check that.

### Before and after

> ✗ In this tutorial, we'll walk through building your first Mosaica map. By the end, you'll have a powerful, interactive visualisation of your data, ready to explore!
>
> ✓ This tutorial is for someone who has never used Mosaica. We'll take the 29,935 places GeoNames lists for Ireland and put them on a map in your browser, where you'll be able to search them by name and filter them by population. The commands take about four minutes to run, and half of that is compiling Mosaica.

> ✗ `point_visibility` names no field to read an access label from, so every place gets the default label, `public`.
>
> ✓ Mosaica decides who may see each place by giving it an access label. Each viewer holds a set of labels and sees only the places that carry one of them. A label usually comes from a column in your data, but we have no such column, so every place gets the default label, `public`. Because every viewer holds `public`, everyone will see every place.

> ✗ Run `mosaica build`.
>
> ✓ Now build the bundle. The build reads every row of `points.parquet` and works out where each place sits on the map, then stores neighbouring places next to each other on disc, so that the server can read any area of the map as one contiguous stretch. The result goes into the `bundle` directory, and from now on the server reads only the bundle, never your Parquet file.

> ✗ One program, `mosaica`, does all the server-side work: it checks a declaration, builds a bundle and serves it. There are six stages, and each one leaves something you can look at.
>
> ✓ *(Cut both, because the outline that follows shows what each stage does.)*
