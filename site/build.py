import re, markdown, html
src = open("../proofcheck/CONSOLIDATED_PROOFS.md").read()
src = src.split("\n", 1)[1]                      # drop the H1; the page header replaces it
intro, rest = src.split("## 0.", 1)
rest = "## 0." + rest
body = markdown.markdown(rest, extensions=["tables", "sane_lists"])
# theorem-like blocks: a paragraph opening with a bold label
kinds = r"(Theorem|Corollary|Lemma|Proposition)"
body = re.sub(r"<p><strong>(" + kinds + r"[^<]*)</strong>", r'<p class="stmt"><strong>\1</strong>', body)
body = re.sub(r"<p><strong>(Proof\.?)</strong>", r'<p class="proof"><em>Proof.</em>', body)
body = re.sub(r"<p><em>Proof\.</em>", r'<p class="proof"><em>Proof.</em>', body)
body = body.replace("∎", '<span class="qed">∎</span>')
body = re.sub(r"<code>([^<]+\.py[^<]*)</code>", r'<code class="file">\1</code>', body)
# sections → anchors + nav
secs = re.findall(r"<h2>(\d+)\. ([^<]+)</h2>", body)
for n, t in secs:
    body = body.replace(f"<h2>{n}. {t}</h2>", f'<h2 id="s{n}"><span class="num">{n}</span>{t}</h2>', 1)
nav = "".join(f'<li><a href="#s{n}"><span class="num">{n}</span>{t}</a></li>' for n, t in secs)
intro_html = markdown.markdown(intro.strip())
tpl = open("template.html").read()
open("proofs.html", "w").write(tpl.replace("{{NAV}}", nav).replace("{{INTRO}}", intro_html).replace("{{BODY}}", body))
print(len(body), [t for _, t in secs])
