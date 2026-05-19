// ed-about.jsx — Screen IV: Rationale / About this paper

function ScreenAbout(){
  return (
    <div className="rationale">
      <div className="eyebrow">Section IV · From the design desk</div>
      <h1>What I changed, and why.</h1>
      <p className="dek">
        A note on the direction taken for this alternative cut of Alto, and the
        principles it is built on.
      </p>

      <div className="epigraph">
        "Information design is a moral act. Density without rhythm is just noise on a screen.
        The newspaper has been a working tool for traders for two hundred years; we have not
        run out of things to learn from it."
      </div>

      <h2>The brief, in one line.</h2>
      <p>
        Build an alternative direction to the warm-dark tactile Alto — not a refinement, not
        a re-skin, but a genuinely opposing interpretation of the same product. Preserve every
        piece of information, the trading-grade density, and the buy/sell colour code; everything
        else is up for negotiation.
      </p>

      <h2>The direction taken.</h2>
      <p>
        <strong>Editorial financial newspaper.</strong> Salmon paper. Source Serif headlines.
        Hairline-ruled tables. Drop caps and bylines. Trading data presented the way the
        <em> Financial Times</em> and the <em>Wall Street Journal</em> have presented it for a
        century — as journalism. Not a dashboard, not a console. A paper.
      </p>

      <h2>Eight decisions.</h2>
      <ul>
        <li>
          <strong>Paper over screen.</strong> The whole shell is a printed object. FT salmon
          (#fff1e5), warm ink (never pure black), three-double-rule masthead, colophon at the
          foot of every section. The visual metaphor isn't "trading app" — it's
          <em> the morning edition</em>.
        </li>
        <li>
          <strong>Type does the work, not chrome.</strong> Source Serif 4 for headlines, IBM Plex
          Sans for UI labels in small caps, IBM Plex Mono only for tabular numerics. There are
          no bevels, no shadows, no rounded corners, no gradient panels. Information separates
          itself with hairline rules and white space.
        </li>
        <li>
          <strong>Watchlist as a stock table.</strong> The watch list is not a row of tiles —
          it's a typeset stock table with newspaper-style subheads ("Megacaps · Technology",
          "Cyclical &amp; Auto"), italic company descriptions, and a 30-session sparkline column.
          It would read correctly on newsprint.
        </li>
        <li>
          <strong>DOM ladder as a paired ledger.</strong> The order book is presented as a
          two-column "Bid / Ask Register" — printed-form lettering, dotted leader rules,
          top-of-book numbers in serif display. The spread line at the foot is a
          newspaper footnote, not a screen widget.
        </li>
        <li>
          <strong>Order ticket as a printed stub.</strong> The order ticket is a physical
          ticket — perforated edges, a serial number (№ TX-2026.05.17-04827), labelled fields
          like a 1970s broker's slip. Buy/Sell uses the desk's editorial inks, not the screen's
          neon. The submit button is full-bleed and committed; this isn't a place to dither.
        </li>
        <li>
          <strong>News as actual journalism.</strong> The Trading Floor's news column has bylines,
          datelines, kickers, and source attribution ("REUTERS · 15:42 ET"). Every wire reads as
          a real dispatch — body copy mixes facts, attribution, and market consequence. Compare
          to the old app, where news was bullet-point body text.
        </li>
        <li>
          <strong>Sector heatmap as a league table.</strong> The bento heatmap becomes a ranked
          ledger with numbered rows, italic sector names, a proportional bar column, and a
          terminal-style percentage. Same data, different rhetoric: this is the FT's
          end-of-day league, not a stock-screener tile.
        </li>
        <li>
          <strong>One claret accent, used sparingly.</strong> Where the original Alto sang in
          warm amber, this version uses a single deep claret — the FT's masthead red — for
          the active section tab, the last-price annotation, the kicker labels, and not much
          else. Buy and sell stay in their proper editorial greens and reds; nothing else is
          allowed to compete with the type.
        </li>
      </ul>

      <h2>What it isn't.</h2>
      <p>
        It is not a Linear-style "calm" SaaS dashboard. It is not a Bloomberg throwback. It is
        not a brutalist black-and-neon terminal. Those are all valid alternative directions —
        this one stakes out a specific stance: <em>traders read a paper every morning; let the
        software feel like it</em>. The Daily is the front page, Markets is the company-page
        deep dive, the Trading Floor is the live wire desk. The voice is consistent across all
        three.
      </p>

      <h2>Who is this for.</h2>
      <p>
        A discretionary trader or PM who values information hierarchy above interactivity —
        someone who prints out the FT, marks it with a fountain pen, and only then sits down
        at the terminal. The shell isn't trying to feel "modern"; it's trying to feel
        <em> considered</em>. Density survives. Edge survives. The metaphor changes.
      </p>

      <div className="rule-double" style={{ marginTop: 36, paddingTop: 18 }}>
        <div style={{ fontFamily:'var(--sans)', fontSize:10, letterSpacing:'.18em',
                       textTransform:'uppercase', color:'var(--ink-2)' }}>
          Design memo · 17 May 2026 · Internal circulation
        </div>
      </div>
    </div>
  );
}

window.ScreenAbout = ScreenAbout;
