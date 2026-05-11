import { useEffect, useRef, useState, type ReactNode } from 'react';
import {
  Btn,
  Field, Input, Textarea, Select, Toggle,
  Surface, SectionHeader, Divider, Kbd,
  Tag, Status,
  Address, Metric, Stat, Skeleton,
  Tabs, type TabItem,
  Code, CodeBlock,
  Alert, Toast, ToastStack, type NotifyTone,
  AppBar, type AppBarItem,
  SideNav, type SideNavGroup,
  Breadcrumbs,
  Pagination,
  CommandPalette, type PaletteGroup,
} from '../components/design';

/* ─── Token tables (read from CSS vars at runtime) ───────────────── */

const SURFACE_TOKENS = [
  { name: 'canvas',          var: '--color-canvas',          desc: 'Outer frame, deepest layer' },
  { name: 'bg',              var: '--color-bg',              desc: 'App background' },
  { name: 'surface',         var: '--color-surface',         desc: 'Default card / input surface' },
  { name: 'surface-raised',  var: '--color-surface-raised',  desc: 'Hover or stacked surface' },
  { name: 'surface-overlay', var: '--color-surface-overlay', desc: 'Modal, popover, focused row' },
  { name: 'surface-sunken',  var: '--color-surface-sunken',  desc: 'Inset wells, code blocks' },
];

const BORDER_TOKENS = [
  { name: 'border',          var: '--color-border',          desc: 'Hairline, default' },
  { name: 'border-strong',   var: '--color-border-strong',   desc: 'Emphasized boundary' },
  { name: 'border-focus',    var: '--color-border-focus',    desc: 'Focus outline' },
];

const FOREGROUND_TOKENS = [
  { name: 'fg',              var: '--color-fg',              desc: 'Primary body text' },
  { name: 'fg-secondary',    var: '--color-fg-secondary',    desc: 'Secondary body, descriptions' },
  { name: 'fg-muted',        var: '--color-fg-muted',        desc: 'Labels, captions' },
  { name: 'fg-faint',        var: '--color-fg-faint',        desc: 'Placeholder, disabled' },
  { name: 'fg-inverse',      var: '--color-fg-inverse',      desc: 'Text on accent fills' },
];

const ACCENT_TOKENS = [
  { name: 'accent',          var: '--color-accent',          desc: 'Primary action, links' },
  { name: 'accent-hover',    var: '--color-accent-hover',    desc: 'Accent on hover' },
  { name: 'accent-pressed',  var: '--color-accent-pressed',  desc: 'Accent on press' },
  { name: 'accent-tint',     var: '--color-accent-tint',     desc: '10% fill, soft tags' },
  { name: 'accent-edge',     var: '--color-accent-edge',     desc: '30% border, soft tags' },
];

const SEMANTIC_TOKENS = [
  { name: 'success',         var: '--color-success',         desc: 'Positive, operational' },
  { name: 'warning',         var: '--color-warning',         desc: 'Caution, pending' },
  { name: 'danger',          var: '--color-danger',          desc: 'Failure, destructive' },
  { name: 'info',            var: '--color-info',            desc: 'Informational, neutral signal' },
];

/* ─── Page ───────────────────────────────────────────────────────── */

const SECTIONS = [
  { id: 'foundations',  label: 'Foundations' },
  { id: 'color',        label: 'Color',       indent: true },
  { id: 'typography',   label: 'Typography',  indent: true },
  { id: 'geometry',     label: 'Geometry',    indent: true },
  { id: 'motion',       label: 'Motion',      indent: true },
  { id: 'components',   label: 'Components' },
  { id: 'buttons',      label: 'Buttons',     indent: true },
  { id: 'inputs',       label: 'Inputs',      indent: true },
  { id: 'tags',         label: 'Tags',        indent: true },
  { id: 'tabs',         label: 'Tabs',        indent: true },
  { id: 'data',         label: 'Data',        indent: true },
  { id: 'surfaces',     label: 'Surfaces',    indent: true },
  { id: 'patterns',     label: 'Patterns' },
  { id: 'address',      label: 'Address',     indent: true },
  { id: 'metric-row',   label: 'Metric row',  indent: true },
  { id: 'empty-state',  label: 'Empty state', indent: true },
  { id: 'domain',       label: 'Domain' },
  { id: 'operators',    label: 'Operators',   indent: true },
  { id: 'wasm',         label: 'Components',  indent: true },
  { id: 'services',     label: 'Services',    indent: true },
  { id: 'events',       label: 'Events',      indent: true },
  { id: 'logs',         label: 'Logs',        indent: true },
  { id: 'feedback',     label: 'Feedback' },
  { id: 'alerts',       label: 'Alerts',      indent: true },
  { id: 'toasts',       label: 'Toasts',      indent: true },
  { id: 'form-errors',  label: 'Form errors', indent: true },
  { id: 'error-state',  label: 'Error state', indent: true },
  { id: 'confirm',      label: 'Confirm',     indent: true },
  { id: 'navigation',   label: 'Navigation' },
  { id: 'app-bar',      label: 'App bar',     indent: true },
  { id: 'side-nav',     label: 'Sidebar',     indent: true },
  { id: 'breadcrumbs',  label: 'Breadcrumbs', indent: true },
  { id: 'pagination',   label: 'Pagination',  indent: true },
  { id: 'palette',      label: 'Palette',     indent: true },
  { id: 'responsive',   label: 'Responsive',  indent: true },
  { id: 'principles',   label: 'Principles' },
];

export function Design() {
  const [active, setActive] = useState<string>('foundations');
  const [navOpen, setNavOpen] = useState(false);

  useEffect(() => {
    const root = document.getElementById('design-scroll');
    if (!root) return;
    const targets = SECTIONS.map((s) => document.getElementById(s.id)).filter(Boolean) as HTMLElement[];
    const obs = new IntersectionObserver(
      (entries) => {
        const visible = entries.filter((e) => e.isIntersecting);
        if (visible.length) {
          const top = visible.reduce((a, b) => (a.boundingClientRect.top < b.boundingClientRect.top ? a : b));
          setActive(top.target.id);
        }
      },
      { root, rootMargin: '-20% 0px -60% 0px', threshold: 0 },
    );
    targets.forEach((t) => obs.observe(t));
    return () => obs.disconnect();
  }, []);

  // Close mobile nav on Escape
  useEffect(() => {
    if (!navOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setNavOpen(false); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navOpen]);

  const scrollTo = (id: string) => {
    const el = document.getElementById(id);
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    setNavOpen(false);
  };

  return (
    <div className="ds h-full overflow-hidden">
      {/* Mobile top bar — only visible below md */}
      <div className="md:hidden sticky top-0 z-30 flex items-center justify-between gap-3 h-12 px-4 border-b border-ink-border bg-ink-bg/90 backdrop-blur-sm">
        <button
          type="button"
          onClick={() => setNavOpen(true)}
          aria-label="Open contents"
          className="inline-flex items-center gap-2 h-8 px-2 rounded-ds-xs text-ink-fg-secondary hover:bg-ink-surface-raised hover:text-ink-fg transition-colors duration-ds-fast cursor-pointer"
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M2 4h10M2 7h10M2 10h10" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
          <span className="text-sm">Contents</span>
        </button>
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted truncate">
          {SECTIONS.find((s) => s.id === active)?.label ?? 'Design'}
        </span>
      </div>

      {/* Mobile drawer backdrop */}
      {navOpen && (
        <button
          type="button"
          aria-label="Close contents"
          onClick={() => setNavOpen(false)}
          className="md:hidden fixed inset-0 z-30 bg-ink-canvas/70 backdrop-blur-[2px] cursor-default"
        />
      )}

      <div className="grid md:grid-cols-[220px_minmax(0,1fr)] h-[calc(100%-3rem)] md:h-full">
        {/* TOC */}
        <aside
          className={[
            'border-r border-ink-border bg-ink-bg overflow-y-auto py-8 px-4',
            'fixed md:sticky inset-y-0 left-0 top-0 z-40 md:z-auto',
            'w-[260px] md:w-auto md:max-h-screen md:self-start',
            'transition-transform duration-ds-base ease-ds',
            navOpen ? 'translate-x-0' : '-translate-x-full md:translate-x-0',
          ].join(' ')}
        >
          <div className="flex items-center justify-between mb-4">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Contents</span>
            <button
              type="button"
              aria-label="Close contents"
              onClick={() => setNavOpen(false)}
              className="md:hidden inline-flex h-7 w-7 items-center justify-center rounded-ds-xs text-ink-fg-muted hover:bg-ink-surface-raised hover:text-ink-fg cursor-pointer"
            >
              <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
                <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
              </svg>
            </button>
          </div>
          <nav className="flex flex-col gap-px">
            {SECTIONS.map((s) => (
              <button
                key={s.id}
                onClick={() => scrollTo(s.id)}
                className={[
                  'text-left text-sm py-1 transition-colors duration-ds-fast cursor-pointer',
                  s.indent ? 'pl-4' : 'pl-2 mt-2 first:mt-0 font-medium',
                  active === s.id ? 'text-ink-accent' : 'text-ink-fg-muted hover:text-ink-fg',
                ].join(' ')}
              >
                {s.label}
              </button>
            ))}
          </nav>
        </aside>

        {/* Content */}
        <main id="design-scroll" className="overflow-y-auto">
          <div className="max-w-[920px] mx-auto px-5 py-10 md:px-12 md:py-16 flex flex-col gap-16 md:gap-24">
            <Hero />

            {/* FOUNDATIONS ──────────────────────────────────────── */}
            <section id="foundations" className="flex flex-col gap-10 scroll-mt-8">
              <SectionHeader
                eyebrow="01"
                title="Foundations"
                description="The atomic design tokens — surfaces, type, geometry, motion. Every component composes from these. Tokens are CSS variables, themeable at runtime."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="color" eyebrow="01.1" title="Color" description="Warm-monochrome palette on graphite. A single accent (electric violet) carries all brand and interaction state. Semantic colors are desaturated — never neon.">
              <ColorGroup heading="Surfaces" tokens={SURFACE_TOKENS} />
              <ColorGroup heading="Borders"  tokens={BORDER_TOKENS} />
              <ColorGroup heading="Foreground" tokens={FOREGROUND_TOKENS} />
              <ColorGroup heading="Accent"     tokens={ACCENT_TOKENS} />
              <ColorGroup heading="Semantic"   tokens={SEMANTIC_TOKENS} />
            </SubSection>

            <SubSection id="typography" eyebrow="01.2" title="Typography" description="IBM Plex Sans for UI, IBM Plex Mono for hashes, addresses, identifiers, and metrics. Plex Serif used sparingly for editorial display only.">
              <TypeSpecimen />
              <TypeScale />
            </SubSection>

            <SubSection id="geometry" eyebrow="01.3" title="Geometry" description="Sharp by default. Cards take 4px. Pills (9999px) reserve themselves for status dots. No drop shadows — depth comes from surface lightness.">
              <RadiiGrid />
              <SpacingGrid />
            </SubSection>

            <SubSection id="motion" eyebrow="01.4" title="Motion" description="Motion confirms causation. Defaults to fast and easeOut. Never decorative.">
              <MotionGrid />
            </SubSection>

            {/* COMPONENTS ──────────────────────────────────────── */}
            <section id="components" className="flex flex-col gap-10 mt-8 scroll-mt-8">
              <SectionHeader
                eyebrow="02"
                title="Components"
                description="Composable primitives. Variant matrices below cover every state we ship."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="buttons" eyebrow="02.1" title="Buttons">
              <ButtonsDemo />
            </SubSection>

            <SubSection id="inputs" eyebrow="02.2" title="Inputs">
              <InputsDemo />
            </SubSection>

            <SubSection id="tags" eyebrow="02.3" title="Tags & Status">
              <TagsDemo />
            </SubSection>

            <SubSection id="tabs" eyebrow="02.4" title="Tabs">
              <TabsDemo />
            </SubSection>

            <SubSection id="data" eyebrow="02.5" title="Data display">
              <DataDemo />
            </SubSection>

            <SubSection id="surfaces" eyebrow="02.6" title="Surfaces & code">
              <SurfacesDemo />
            </SubSection>

            {/* PATTERNS ────────────────────────────────────────── */}
            <section id="patterns" className="flex flex-col gap-10 mt-8 scroll-mt-8">
              <SectionHeader
                eyebrow="03"
                title="Patterns"
                description="Composed examples — recurring layouts the app needs over and over."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="address" eyebrow="03.1" title="Address & identity">
              <AddressPattern />
            </SubSection>

            <SubSection id="metric-row" eyebrow="03.2" title="Metric row">
              <MetricRowPattern />
            </SubSection>

            <SubSection id="empty-state" eyebrow="03.3" title="Empty & loading state">
              <EmptyStatePattern />
            </SubSection>

            {/* DOMAIN ──────────────────────────────────────────── */}
            <section id="domain" className="flex flex-col gap-10 mt-8 scroll-mt-8">
              <SectionHeader
                eyebrow="04"
                title="Domain"
                description="The four entity types every WAVS operator works with: people running the network, code they run, services they orchestrate, and events those services produce. Each gets a list view, a detail surface, and a representative empty state."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="operators" eyebrow="04.1" title="Operators" description="Node operators participating in consensus. Identity is peer-id-first (libp2p), with optional human label. Performance is signed-vs-missed; staleness is the most diagnostic signal.">
              <OperatorsPattern />
            </SubSection>

            <SubSection id="wasm" eyebrow="04.2" title="Components" description="WASM modules in the registry, addressed by digest. Components are immutable — versioning is a function of which digest a service points at.">
              <ComponentsPattern />
            </SubSection>

            <SubSection id="services" eyebrow="04.3" title="Services" description="Deployed AVS services. A service binds a service-manager contract, a component digest, and a trigger source. Service detail uses sub-tab navigation for dense, role-specific views.">
              <ServicesPattern />
            </SubSection>

            <SubSection id="events" eyebrow="04.4" title="Events" description="Triggers, executions, signatures, submissions — the live wire of the network. Two read patterns: tailing (debugging, real-time) and tabular (audit, paginated).">
              <EventsPattern />
            </SubSection>

            <SubSection id="logs" eyebrow="04.5" title="Logs" description="Diagnostic output — structured tracing emissions from the node. Level is color, not row. Fields are key=value, monospace, scannable. Follow-tail is the default; scroll up to pause.">
              <LogsPattern />
            </SubSection>

            {/* FEEDBACK ────────────────────────────────────────── */}
            <section id="feedback" className="flex flex-col gap-10 mt-8 scroll-mt-8">
              <SectionHeader
                eyebrow="05"
                title="Feedback"
                description="How the system tells the operator something happened — or didn't. Persistent alerts for state, transient toasts for events, and explicit confirmations for irreversible action. Errors are first-class; they show as much as they hide."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="alerts" eyebrow="05.1" title="Alerts" description="Persistent, in-context. Inline alerts sit inside content; banners stretch full-width across a page or surface.">
              <AlertsPattern />
            </SubSection>

            <SubSection id="toasts" eyebrow="05.2" title="Toasts" description="Transient confirmations. Auto-dismiss after 5s by default, but errors stay until acknowledged. Stack from the bottom-right.">
              <ToastsPattern />
            </SubSection>

            <SubSection id="form-errors" eyebrow="05.3" title="Form errors" description="Validation lives at three levels: per-field, per-form summary, and submit-time error rejection. The field is always the source of truth.">
              <FormErrorsPattern />
            </SubSection>

            <SubSection id="error-state" eyebrow="05.4" title="Error state" description="When a whole surface fails — load failed, peer disconnected, RPC down. Show the cause, the impact, and the next move.">
              <ErrorStatePattern />
            </SubSection>

            <SubSection id="confirm" eyebrow="05.5" title="Confirmation" description="For destructive or irreversible actions: pause, restate, require explicit acknowledgment.">
              <ConfirmPattern />
            </SubSection>

            {/* NAVIGATION ──────────────────────────────────────── */}
            <section id="navigation" className="flex flex-col gap-10 mt-8 scroll-mt-8">
              <SectionHeader
                eyebrow="06"
                title="Navigation"
                description="How operators move through the app. Top app bar carries primary destinations; sidebars carry sub-navigation; breadcrumbs anchor location; the command palette is the keyboardist's shortcut. Every primitive collapses gracefully below the md breakpoint."
                level={1}
              />
              <Divider />
            </section>

            <SubSection id="app-bar" eyebrow="06.1" title="App bar" description="Primary horizontal navigation. Shows brand, destinations, and global actions. Below md, items collapse into a hamburger dropdown; on tight desktop layouts, switch to compact (icon-only) mode.">
              <AppBarPattern />
            </SubSection>

            <SubSection id="side-nav" eyebrow="06.2" title="Sidebar" description="Vertical navigation, optionally grouped. Has a collapsed (icon-only) state that animates between 56px and 224px. Group labels disappear when collapsed; tooltips take over on hover.">
              <SideNavPattern />
            </SubSection>

            <SubSection id="breadcrumbs" eyebrow="06.3" title="Breadcrumbs" description="A trail of where you are. Truncates the middle when path depth exceeds the maxItems threshold. The last item is the current page (non-clickable).">
              <BreadcrumbsPattern />
            </SubSection>

            <SubSection id="pagination" eyebrow="06.4" title="Pagination" description="For paginated data tables. Shows a windowed page list, page-range counter, and prev/next controls. Page numbers ellipsize when total exceeds 7 pages.">
              <PaginationPattern />
            </SubSection>

            <SubSection id="palette" eyebrow="06.5" title="Command palette" description="The keyboardist's interface. Opens with ⌘K. Fuzzy-searches across destinations, services, components, and operators. Arrow keys to navigate, return to select, escape to dismiss.">
              <PalettePattern />
            </SubSection>

            <SubSection id="responsive" eyebrow="06.6" title="Responsive" description="The breakpoint contract. Default Tailwind breakpoints; desktop-first content with mobile drawers and stacking. The /design page itself follows these rules — narrow this window to test.">
              <ResponsivePattern />
            </SubSection>

            {/* PRINCIPLES ──────────────────────────────────────── */}
            <section id="principles" className="flex flex-col gap-10 mt-8 mb-32 scroll-mt-8">
              <SectionHeader
                eyebrow="07"
                title="Principles"
                description="What we believe. When tokens conflict with these, the principles win."
                level={1}
              />
              <Divider />
              <Principles />
            </section>
          </div>
        </main>
      </div>
    </div>
  );
}

/* ─── Hero ──────────────────────────────────────────────────────── */

function Hero() {
  return (
    <header className="flex flex-col gap-6 pb-10 border-b border-ink-border">
      <div className="flex items-center gap-3">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">
          WAVS / Design system
        </span>
        <Tag tone="accent" mono uppercase>v0.1 · draft</Tag>
      </div>
      <h1 className="text-3xl font-medium text-ink-fg max-w-[14ch] leading-[1.05]">
        A quiet interface for verifiable compute.
      </h1>
      <p className="text-md text-ink-fg-secondary max-w-prose">
        The design system for WAVS. Built for operators, researchers, and protocol engineers — people who trust their tools to recede until they're needed. Warm graphite, plex-typeset, and disciplined to the millimetre.
      </p>
      <div className="flex items-center gap-3 mt-2">
        <Status tone="live" label="Tokens stable" />
        <span className="text-ink-border-strong">·</span>
        <Status tone="pending" label="Migration in progress" />
        <span className="text-ink-border-strong">·</span>
        <span className="font-mono text-xs text-ink-fg-muted">7 components / 5 patterns</span>
      </div>
    </header>
  );
}

/* ─── Subsection wrapper ────────────────────────────────────────── */

function SubSection({
  id, eyebrow, title, description, children,
}: { id: string; eyebrow: string; title: string; description?: string; children: ReactNode }) {
  return (
    <section id={id} className="flex flex-col gap-6 scroll-mt-8">
      <SectionHeader eyebrow={eyebrow} title={title} description={description} level={2} />
      <div className="flex flex-col gap-6">{children}</div>
    </section>
  );
}

/* ─── Color ─────────────────────────────────────────────────────── */

function ColorGroup({ heading, tokens }: { heading: string; tokens: typeof SURFACE_TOKENS }) {
  return (
    <div className="flex flex-col gap-3">
      <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{heading}</div>
      <Surface variant="outline" className="overflow-hidden">
        <table className="w-full text-sm">
          <tbody>
            {tokens.map((t, i) => (
              <tr key={t.var} className={i > 0 ? 'border-t border-ink-border' : ''}>
                <td className="w-12 p-0 align-middle">
                  <div className="h-12 w-full" style={{ backgroundColor: `var(${t.var})` }} aria-hidden />
                </td>
                <td className="px-4 py-3 align-middle">
                  <div className="font-mono text-sm text-ink-fg">{t.name}</div>
                </td>
                <td className="px-4 py-3 align-middle">
                  <span className="font-mono text-xs text-ink-fg-muted">{t.var}</span>
                </td>
                <td className="px-4 py-3 text-sm text-ink-fg-secondary align-middle">{t.desc}</td>
                <td className="px-4 py-3 align-middle text-right">
                  <HexProbe cssVar={t.var} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Surface>
    </div>
  );
}

function HexProbe({ cssVar }: { cssVar: string }) {
  const [hex, setHex] = useState<string>('');
  useEffect(() => {
    const v = getComputedStyle(document.documentElement).getPropertyValue(cssVar).trim();
    setHex(v);
  }, [cssVar]);
  return <span className="font-mono text-xs text-ink-fg-faint">{hex}</span>;
}

/* ─── Typography ────────────────────────────────────────────────── */

function TypeSpecimen() {
  return (
    <Surface className="p-8 flex flex-col gap-6 bg-ink-canvas">
      <div className="flex flex-col gap-3 items-start">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Plex Sans · Display</span>
        <span className="text-3xl text-ink-fg leading-tight">Verifiable, off-chain.</span>
      </div>
      <Divider />
      <div className="flex flex-col gap-3 items-start">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Plex Sans · Body</span>
        <p className="text-md text-ink-fg-secondary max-w-prose">
          WAVS executes Actively Validated Service logic as sandboxed WebAssembly components, bridging blockchain events with off-chain computation and coordinating multi-operator consensus.
        </p>
      </div>
      <Divider />
      <div className="flex flex-col gap-3 items-start">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Plex Mono · Identifier</span>
        <span className="font-mono text-md text-ink-fg">0xa78b·fa6f·c4b0·9b7d</span>
      </div>
      <Divider />
      <div className="flex flex-col gap-3 items-start">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Plex Serif · Editorial</span>
        <span className="font-serif text-xl italic text-ink-fg-secondary">"Don't trust, verify."</span>
      </div>
    </Surface>
  );
}

const TYPE_SCALE: { name: string; size: string; lineHeight: string; tw: string }[] = [
  { name: 'xs',   size: '11', lineHeight: '16', tw: 'text-xs' },
  { name: 'sm',   size: '12', lineHeight: '18', tw: 'text-sm' },
  { name: 'base', size: '13', lineHeight: '20', tw: 'text-base' },
  { name: 'md',   size: '14', lineHeight: '22', tw: 'text-md' },
  { name: 'lg',   size: '16', lineHeight: '24', tw: 'text-lg' },
  { name: 'xl',   size: '20', lineHeight: '28', tw: 'text-xl' },
  { name: '2xl',  size: '28', lineHeight: '34', tw: 'text-2xl' },
  { name: '3xl',  size: '40', lineHeight: '46', tw: 'text-3xl' },
];

function TypeScale() {
  return (
    <Surface variant="outline" className="overflow-hidden">
      <table className="w-full">
        <tbody>
          {TYPE_SCALE.map((t, i) => (
            <tr key={t.name} className={i > 0 ? 'border-t border-ink-border' : ''}>
              <td className="px-4 py-4 w-24 align-baseline">
                <span className="font-mono text-xs text-ink-fg-muted">{t.tw}</span>
              </td>
              <td className="px-4 py-4 w-32 align-baseline">
                <span className="font-mono text-xs text-ink-fg-faint">{t.size}/{t.lineHeight}</span>
              </td>
              <td className="px-4 py-4 align-baseline">
                <span className={`${t.tw} text-ink-fg`}>The quick brown fox</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Surface>
  );
}

/* ─── Geometry ──────────────────────────────────────────────────── */

const RADII = [
  { name: 'none', tw: 'rounded-ds-none', value: '0px',    use: 'Data tables, code blocks' },
  { name: 'xs',   tw: 'rounded-ds-xs',   value: '2px',    use: 'Buttons, inputs, tags' },
  { name: 'sm',   tw: 'rounded-ds-sm',   value: '4px',    use: 'Cards, surfaces' },
  { name: 'md',   tw: 'rounded-ds-md',   value: '6px',    use: 'Modals, popovers' },
  { name: 'lg',   tw: 'rounded-ds-lg',   value: '10px',   use: 'Sparingly — large containers' },
  { name: 'pill', tw: 'rounded-ds-pill', value: '9999px', use: 'Status dots only' },
];

function RadiiGrid() {
  return (
    <div>
      <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-3">Radii</div>
      <Surface variant="outline" className="overflow-hidden">
        <table className="w-full">
          <tbody>
            {RADII.map((r, i) => (
              <tr key={r.name} className={i > 0 ? 'border-t border-ink-border' : ''}>
                <td className="px-4 py-3 w-24">
                  <div className={`h-7 w-12 bg-ink-accent ${r.tw}`} />
                </td>
                <td className="px-4 py-3 w-24">
                  <span className="font-mono text-sm text-ink-fg">{r.name}</span>
                </td>
                <td className="px-4 py-3 w-24">
                  <span className="font-mono text-xs text-ink-fg-muted">{r.value}</span>
                </td>
                <td className="px-4 py-3 text-sm text-ink-fg-secondary">{r.use}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Surface>
    </div>
  );
}

const SPACING = [
  { name: '1',  px: 4 },
  { name: '2',  px: 8 },
  { name: '3',  px: 12 },
  { name: '4',  px: 16 },
  { name: '6',  px: 24 },
  { name: '8',  px: 32 },
  { name: '12', px: 48 },
  { name: '16', px: 64 },
];

function SpacingGrid() {
  return (
    <div>
      <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-3">Spacing</div>
      <Surface variant="outline" className="p-6 flex flex-col gap-3">
        {SPACING.map((s) => (
          <div key={s.name} className="flex items-center gap-4">
            <span className="font-mono text-xs text-ink-fg-muted w-10">{s.name}</span>
            <span className="font-mono text-xs text-ink-fg-faint w-10">{s.px}</span>
            <span className="block h-2 bg-ink-accent" style={{ width: s.px }} />
          </div>
        ))}
      </Surface>
    </div>
  );
}

/* ─── Motion ────────────────────────────────────────────────────── */

function MotionGrid() {
  const [tick, setTick] = useState(0);
  return (
    <Surface variant="outline" className="p-8">
      <div className="flex flex-col gap-6">
        <div className="flex items-center justify-between">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Easing samples</span>
          <Btn size="sm" variant="ghost" onClick={() => setTick((t) => t + 1)}>Replay ↻</Btn>
        </div>
        {[
          { name: 'instant', dur: '80ms',  ease: 'easeOut',   tw: 'duration-ds-instant' },
          { name: 'fast',    dur: '140ms', ease: 'easeOut',   tw: 'duration-ds-fast' },
          { name: 'base',    dur: '200ms', ease: 'easeOut',   tw: 'duration-ds-base' },
          { name: 'slow',    dur: '320ms', ease: 'easeOut',   tw: 'duration-ds-slow' },
        ].map((m) => (
          <div key={m.name} className="grid grid-cols-[80px_80px_1fr] items-center gap-4">
            <span className="font-mono text-xs text-ink-fg-secondary">{m.name}</span>
            <span className="font-mono text-xs text-ink-fg-muted">{m.dur}</span>
            <div className="relative h-1.5 bg-ink-surface-sunken rounded-ds-pill overflow-hidden">
              <span
                key={`${m.name}-${tick}`}
                className={`absolute top-0 left-0 h-full w-full bg-ink-accent transition-transform ease-ds ${m.tw}`}
                style={{ transform: `translateX(-100%)`, animation: `slideIn ${m.dur} var(--ease-out) forwards` }}
              />
            </div>
          </div>
        ))}
      </div>
      <style>{`@keyframes slideIn { from { transform: translateX(-100%); } to { transform: translateX(0); } }`}</style>
    </Surface>
  );
}

/* ─── Buttons demo ──────────────────────────────────────────────── */

function ButtonsDemo() {
  return (
    <>
      <DemoMatrix
        rows={[
          { key: 'primary',   label: 'Primary' },
          { key: 'secondary', label: 'Secondary' },
          { key: 'ghost',     label: 'Ghost' },
          { key: 'danger',    label: 'Danger' },
        ]}
        cols={[
          { key: 'sm',  label: 'sm' },
          { key: 'md',  label: 'md' },
          { key: 'lg',  label: 'lg' },
          { key: 'disabled', label: 'disabled' },
          { key: 'loading',  label: 'loading' },
        ]}
        render={(rowKey, colKey) => {
          const variant = rowKey as 'primary' | 'secondary' | 'ghost' | 'danger';
          if (colKey === 'disabled') return <Btn variant={variant} disabled>Action</Btn>;
          if (colKey === 'loading')  return <Btn variant={variant} loading>Action</Btn>;
          return <Btn variant={variant} size={colKey as 'sm' | 'md' | 'lg'}>Action</Btn>;
        }}
      />
      <div className="flex flex-wrap gap-3">
        <Btn variant="primary" leading={<PlusIcon />}>Deploy service</Btn>
        <Btn variant="secondary" trailing={<ArrowIcon />}>Continue</Btn>
        <Btn variant="ghost" leading={<RefreshIcon />}>Reload</Btn>
        <Btn variant="danger" leading={<TrashIcon />}>Reset all data</Btn>
      </div>
    </>
  );
}

/* ─── Inputs demo ───────────────────────────────────────────────── */

function InputsDemo() {
  const [text, setText] = useState('0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80');
  const [enabled, setEnabled] = useState(true);
  return (
    <Surface variant="outline" className="p-8">
      <div className="grid grid-cols-2 gap-x-8 gap-y-6">
        <Field label="Service name" hint="Lowercase, no spaces" id="f1">
          <Input id="f1" placeholder="my-avs-service" />
        </Field>
        <Field label="Operator address" id="f2">
          <Input id="f2" mono value={text} onChange={setText} leading={<HashIcon />} />
        </Field>
        <Field label="API key" optional id="f3">
          <Input id="f3" type="password" placeholder="sk-…" />
        </Field>
        <Field label="Environment" id="f4">
          <Select
            id="f4"
            value="mainnet"
            options={[
              { value: 'mainnet', label: 'Mainnet' },
              { value: 'sepolia', label: 'Sepolia' },
              { value: 'holesky', label: 'Holesky' },
              { value: 'local',   label: 'Local · Anvil' },
            ]}
          />
        </Field>
        <Field label="Component config" hint="TOML or JSON. Submit with ⌘↵" id="f5" className="col-span-2">
          <Textarea
            id="f5"
            mono
            rows={5}
            defaultValue={`[component]\nname = "echo"\ndigest = "sha256:a78bfa6f…"\n`}
          />
        </Field>
        <Field label="Validation error" id="f6" error="Invalid checksum: expected sha256, got blake3.">
          <Input id="f6" invalid value="blake3:c4b0…" mono />
        </Field>
        <Field label="Read-only" id="f7">
          <Input id="f7" readOnly value="auto-derived" />
        </Field>
      </div>
      <Divider className="my-8" />
      <div className="flex flex-col gap-4">
        <Toggle
          checked={enabled}
          onChange={setEnabled}
          label="Aggregator enabled"
          description="Collect signatures from peer operators before submission."
        />
        <Toggle
          checked={false}
          onChange={() => undefined}
          label="Cosmos submission"
          description="Route results to the Cosmos chain in addition to EVM."
        />
        <Toggle
          checked={false}
          onChange={() => undefined}
          disabled
          label="Bring-your-own-RPC"
          description="Coming soon."
        />
      </div>
    </Surface>
  );
}

/* ─── Tags demo ─────────────────────────────────────────────────── */

function TagsDemo() {
  const tones: ('neutral' | 'accent' | 'success' | 'warning' | 'danger' | 'info')[] = [
    'neutral', 'accent', 'success', 'warning', 'danger', 'info',
  ];
  return (
    <>
      <DemoMatrix
        rows={tones.map((t) => ({ key: t, label: t }))}
        cols={[
          { key: 'soft',    label: 'soft' },
          { key: 'solid',   label: 'solid' },
          { key: 'outline', label: 'outline' },
          { key: 'mono',    label: 'mono · uc' },
        ]}
        render={(rowKey, colKey) => {
          const tone = rowKey as typeof tones[number];
          if (colKey === 'mono') return <Tag tone={tone} mono uppercase>v0.4.2</Tag>;
          return (
            <Tag tone={tone} variant={colKey as 'soft' | 'solid' | 'outline'}>
              {rowKey}
            </Tag>
          );
        }}
      />
      <div className="flex items-center gap-6 flex-wrap">
        <Status tone="live" />
        <Status tone="pending" />
        <Status tone="error" />
        <Status tone="paused" />
        <Status tone="idle" />
        <Status tone="live" label="Block 19,847,221" />
        <Status tone="pending" label="Aggregating 3/5" />
      </div>
    </>
  );
}

/* ─── Tabs demo ─────────────────────────────────────────────────── */

function TabsDemo() {
  const tabs: TabItem[] = [
    { key: 'overview', label: 'Overview' },
    { key: 'triggers', label: 'Triggers',    badge: <Tag tone="accent" mono>12</Tag> },
    { key: 'submissions', label: 'Submissions', badge: <Tag tone="warning" mono>2</Tag> },
    { key: 'logs',     label: 'Logs' },
    { key: 'archived', label: 'Archived', disabled: true },
  ];
  const [a, setA] = useState('overview');
  const [b, setB] = useState('overview');
  return (
    <Surface variant="outline" className="p-6 flex flex-col gap-8">
      <div>
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-3">Underline · navigation</div>
        <Tabs items={tabs} active={a} onChange={setA} />
      </div>
      <div>
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-3">Segmented · filter</div>
        <Tabs items={tabs.slice(0, 4)} active={b} onChange={setB} variant="segmented" />
      </div>
    </Surface>
  );
}

/* ─── Data demo ─────────────────────────────────────────────────── */

function DataDemo() {
  return (
    <Surface variant="outline" className="p-8 flex flex-col gap-8">
      <div className="grid grid-cols-4 gap-6">
        <Metric label="Block height"  value="19,847,221" />
        <Metric label="Operators"     value="14" unit="online" delta={{ value: '+2', direction: 'up' }} />
        <Metric label="Avg. latency"  value="142" unit="ms" delta={{ value: '12 ms', direction: 'down' }} hint="last hour" />
        <Metric label="Failed runs"   value="0" delta={{ value: '0', direction: 'flat' }} />
      </div>
      <Divider />
      <div className="grid grid-cols-2 gap-x-8">
        <div>
          <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-2">Service detail</div>
          <Stat label="Service ID"    value="svc-7af2b1e0" />
          <Stat label="Chain"         value="Ethereum / Mainnet" />
          <Stat label="Component"     value={<Address value="sha256:a78bfa6fc4b09b7dde2a1c0f9b3e6d05" />} />
          <Stat label="Manager"       value={<Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" />} />
          <Stat label="Status"        value={<Status tone="live" />} mono={false} />
          <Stat label="Last trigger"  value="2026-04-28 14:22:08" />
        </div>
        <div>
          <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-2">Loading…</div>
          <div className="flex flex-col gap-2 mt-2">
            <Skeleton width="40%" height={14} />
            <Skeleton width="80%" height={14} />
            <Skeleton width="65%" height={14} />
            <Skeleton width="50%" height={14} />
          </div>
        </div>
      </div>
    </Surface>
  );
}

/* ─── Surfaces / code demo ──────────────────────────────────────── */

function SurfacesDemo() {
  return (
    <div className="flex flex-col gap-6">
      <div className="grid grid-cols-4 gap-3">
        {(['flat', 'raised', 'sunken', 'outline'] as const).map((v) => (
          <Surface key={v} variant={v} className="p-5 h-24 flex flex-col items-start justify-between">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{v}</span>
            <span className="text-sm text-ink-fg-secondary">Surface</span>
          </Surface>
        ))}
      </div>
      <CodeBlock language="rust">
{`// trigger handler — runs in a Wasmtime WASI sandbox
pub fn handle(event: TriggerEvent) -> Result<Output> {
    let payload = event.decode::<TaskRequest>()?;
    let response = compute(&payload).await?;
    Ok(Output::evm(response.encode()))
}`}
      </CodeBlock>
      <div className="flex items-center gap-3 flex-wrap">
        <span className="text-sm text-ink-fg-secondary">Press</span>
        <Kbd>⌘</Kbd>
        <Kbd>K</Kbd>
        <span className="text-sm text-ink-fg-secondary">to open the command palette, or</span>
        <Kbd>g</Kbd>
        <span className="text-sm text-ink-fg-muted">then</span>
        <Kbd>s</Kbd>
        <span className="text-sm text-ink-fg-secondary">to jump to services.</span>
      </div>
    </div>
  );
}

/* ─── Patterns ──────────────────────────────────────────────────── */

function AddressPattern() {
  return (
    <Surface variant="outline" className="p-8 flex flex-col gap-5">
      <div className="flex items-center gap-3 flex-wrap">
        <Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" />
        <Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" truncate={4} />
        <Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" truncate={false} />
        <Address value="sha256:a78bfa6fc4b09b7dde2a1c0f9b3e6d05" />
      </div>
      <Divider />
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="h-9 w-9 rounded-ds-xs bg-ink-accent-tint border border-ink-accent-edge flex items-center justify-center">
            <span className="font-mono text-xs text-ink-accent">SR</span>
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-sm text-ink-fg">Stake Registry</span>
            <Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" />
          </div>
        </div>
        <Tag tone="accent" mono uppercase>verified</Tag>
      </div>
    </Surface>
  );
}

function MetricRowPattern() {
  return (
    <Surface variant="flat" className="overflow-hidden">
      <div className="flex items-center justify-between px-6 py-4 border-b border-ink-border">
        <div className="flex items-center gap-3">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Service</span>
          <span className="text-sm text-ink-fg">price-oracle-mainnet</span>
          <Status tone="live" />
        </div>
        <Btn size="sm" variant="ghost" trailing={<ArrowIcon />}>Open</Btn>
      </div>
      <div className="grid grid-cols-4 divide-x divide-ink-border">
        <Metric className="p-5"  label="Triggers / hr"  value="1,284"    size="sm" />
        <Metric className="p-5"  label="Median latency" value="142" unit="ms" size="sm" />
        <Metric className="p-5"  label="Operators"      value="14 / 14"  size="sm" />
        <Metric className="p-5"  label="Last block"     value="19,847,221" size="sm" />
      </div>
    </Surface>
  );
}

function EmptyStatePattern() {
  return (
    <Surface variant="outline" className="p-12 flex flex-col items-center text-center gap-4">
      <div className="h-12 w-12 rounded-ds-sm border border-dashed border-ink-border-strong flex items-center justify-center">
        <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
          <path d="M4 6h12M4 10h12M4 14h8" stroke="currentColor" strokeWidth="1.2" className="text-ink-fg-faint" />
        </svg>
      </div>
      <div className="flex flex-col gap-1.5 max-w-sm">
        <h3 className="text-md text-ink-fg">No services deployed</h3>
        <p className="text-sm text-ink-fg-muted">
          Deploy your first WAVS service to begin processing triggers. Components are loaded from the local registry.
        </p>
      </div>
      <div className="flex items-center gap-2 mt-2">
        <Btn variant="primary" leading={<PlusIcon />}>Deploy a service</Btn>
        <Btn variant="ghost">Read the docs</Btn>
      </div>
    </Surface>
  );
}

/* ─── Principles ────────────────────────────────────────────────── */

const PRINCIPLES = [
  {
    n: '01',
    title: 'Recede until needed.',
    body: 'The chrome is quiet. The data is loud. Operators will spend hours in this app — anything decorative becomes noise. When in doubt, remove.',
  },
  {
    n: '02',
    title: 'Numbers earn the mono font.',
    body: 'Hashes, addresses, byte counts, prices, latency, counts. If a human will compare it digit-by-digit, it gets a tabular monospace.',
  },
  {
    n: '03',
    title: 'One accent. Many shades of nothing.',
    body: 'A single hue carries every interactive surface. Semantic colors are for state changes, never for hierarchy. Hierarchy is luminance.',
  },
  {
    n: '04',
    title: 'No drop shadows. Depth is light.',
    body: 'Shadows belong to a paper world. Our world is graphite — we move up the lightness ramp to lift a surface, never blur a halo around it.',
  },
  {
    n: '05',
    title: 'Honest motion.',
    body: 'Animation confirms causation, never invents it. Default fast (140ms). Reserve slow easing for spatial transforms only.',
  },
  {
    n: '06',
    title: 'Verifiable by inspection.',
    body: 'Affordances are obvious. Truncated values reveal full content on hover or copy. State is visible, never inferred. The interface tells the truth.',
  },
];

function Principles() {
  return (
    <div className="grid grid-cols-2 gap-px bg-ink-border border border-ink-border rounded-ds-sm overflow-hidden">
      {PRINCIPLES.map((p) => (
        <div key={p.n} className="bg-ink-bg p-6 flex flex-col gap-2 min-h-[170px]">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">{p.n}</span>
          <h3 className="text-md text-ink-fg">{p.title}</h3>
          <p className="text-sm text-ink-fg-secondary leading-relaxed">{p.body}</p>
        </div>
      ))}
    </div>
  );
}

/* ─── Demo matrix utility ───────────────────────────────────────── */

function DemoMatrix({
  rows, cols, render,
}: {
  rows: { key: string; label: string }[];
  cols: { key: string; label: string }[];
  render: (rowKey: string, colKey: string) => ReactNode;
}) {
  return (
    <Surface variant="outline" className="overflow-hidden">
      <table className="w-full">
        <thead>
          <tr className="border-b border-ink-border bg-ink-surface-sunken">
            <th className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted w-32">
              variant / size
            </th>
            {cols.map((c) => (
              <th key={c.key} className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted">
                {c.label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r, i) => (
            <tr key={r.key} className={i > 0 ? 'border-t border-ink-border' : ''}>
              <td className="px-4 py-3 font-mono text-xs uppercase tracking-widest text-ink-fg-secondary">{r.label}</td>
              {cols.map((c) => (
                <td key={c.key} className="px-4 py-3">{render(r.key, c.key)}</td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </Surface>
  );
}

/* ─── Domain — Operators ────────────────────────────────────────── */

interface OperatorRow {
  peerId: string;
  label: string;
  role: 'lead' | 'member';
  stake: string;
  signed: number;
  missed: number;
  lastSeenSec: number;
  trend: number[];
}

const OPERATORS: OperatorRow[] = [
  { peerId: '12D3KooWQYhDdR9k4n5K8z2vYcL7p3qA6mWxBfTuEsHj1cR2dF8x', label: 'coinbase-cloud',   role: 'lead',   stake: '4,200', signed: 7294, missed: 2,  lastSeenSec: 12,  trend: [4,5,5,4,5,5,5,5,5,5,5,5] },
  { peerId: '12D3KooWBcdEf8u92xNh4Y5jK7vZcL9q3pAr6mWxBfTuEsHj1c2D', label: 'figment',          role: 'member', stake: '1,820', signed: 7289, missed: 7,  lastSeenSec: 24,  trend: [5,4,5,5,4,5,5,5,4,5,5,5] },
  { peerId: '12D3KooWPnq3vFXdR8K5L6t9nZ7HhJ2bC4mWxBfTuEsHj1cR2dF7', label: 'nethermind',       role: 'member', stake: '1,640', signed: 7280, missed: 16, lastSeenSec: 9,   trend: [5,5,5,3,5,5,5,4,5,5,5,5] },
  { peerId: '12D3KooWZj7kL5HpR2X9m3vYcL8qNh4Y5jK6vZcL9q3pAr6mW1Bf', label: 'kiln-finance',     role: 'member', stake: '1,200', signed: 7290, missed: 6,  lastSeenSec: 18,  trend: [5,5,4,5,5,5,5,5,5,5,4,5] },
  { peerId: '12D3KooWAr6mWxBfTuEsHj1cR2dF8xQYhDdR9k4n5K8z2vYcL7pK', label: 'p2p.org',          role: 'member', stake: '980',   signed: 7271, missed: 25, lastSeenSec: 41,  trend: [4,5,5,5,3,5,4,5,5,4,5,5] },
  { peerId: '12D3KooWHj1cR2dF8xPnq3vFXdR8K5L6t9nZ7HhJ2bC4mWxBfTuE', label: 'staked.us',        role: 'member', stake: '740',   signed: 7287, missed: 9,  lastSeenSec: 14,  trend: [5,5,5,5,5,5,5,4,5,5,5,5] },
  { peerId: '12D3KooWmWxBfTuEsHj1cR2dF8xQYhDdR9k4n5K8z2vYcL7p3qA6', label: 'allnodes',         role: 'member', stake: '620',   signed: 7283, missed: 13, lastSeenSec: 7,   trend: [5,5,4,5,5,4,5,5,5,5,4,5] },
  { peerId: '12D3KooWnZ7HhJ2bC4mWxBfTuEsHj1cR2dF8xPnq3vFXdR8K5L6t', label: 'chorus-one',       role: 'member', stake: '560',   signed: 7268, missed: 28, lastSeenSec: 88,  trend: [4,5,5,3,5,5,5,4,3,5,5,4] },
  { peerId: '12D3KooWLj1cR2dF8xPnq3vFXdR8K5L6t9nZ7HhJ2bC4mWxBfTuE', label: 'blockdaemon',      role: 'member', stake: '440',   signed: 7295, missed: 1,  lastSeenSec: 5,   trend: [5,5,5,5,5,5,5,5,5,5,5,5] },
  { peerId: '12D3KooW8z2vYcL7p3qA6mWxBfTuEsHj1cR2dF8xQYhDdR9k4n5K', label: 'unlabeled-peer',   role: 'member', stake: '120',   signed: 7124, missed: 172,lastSeenSec: 412, trend: [3,4,5,2,4,1,3,5,3,4,2,3] },
];

function relativeTime(sec: number): string {
  if (sec < 60) return `${sec}s ago`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ago`;
  return `${Math.floor(sec / 3600)}h ago`;
}

function staleness(sec: number): 'live' | 'pending' | 'error' | 'paused' {
  if (sec < 30) return 'live';
  if (sec < 90) return 'pending';
  return 'error';
}

function Sparkline({ values, width = 80, height = 24, tone = 'accent' }: { values: number[]; width?: number; height?: number; tone?: 'accent' | 'success' | 'danger' | 'fg-muted' }) {
  if (values.length < 2) return null;
  const max = Math.max(...values, 1);
  const min = Math.min(...values, 0);
  const range = max - min || 1;
  const stepX = width / (values.length - 1);
  const points = values
    .map((v, i) => `${i * stepX},${height - ((v - min) / range) * height}`)
    .join(' ');
  const colorClass = tone === 'success' ? 'text-ink-success'
    : tone === 'danger' ? 'text-ink-danger'
    : tone === 'fg-muted' ? 'text-ink-fg-muted'
    : 'text-ink-accent';
  return (
    <svg width={width} height={height} className={colorClass} aria-hidden>
      <polyline points={points} fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function OperatorsPattern() {
  return (
    <div className="flex flex-col gap-6">
      {/* Roster header + filter bar */}
      <Surface variant="flat" className="overflow-hidden">
        <div className="flex items-center justify-between gap-4 px-5 py-3 border-b border-ink-border">
          <div className="flex items-center gap-3">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Roster</span>
            <Tag tone="neutral" mono>10 active</Tag>
            <Tag tone="success" mono>9 healthy</Tag>
            <Tag tone="warning" mono>1 stale</Tag>
          </div>
          <div className="flex items-center gap-2">
            <Input
              leading={<SearchIcon />}
              placeholder="Filter by peer-id or label"
              className="w-64"
            />
            <Btn size="sm" variant="ghost" leading={<RefreshIcon />}>Refresh</Btn>
          </div>
        </div>
        <table className="w-full">
          <thead>
            <tr className="text-left bg-ink-surface-sunken border-b border-ink-border">
              <th className="px-5 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Operator</th>
              <th className="px-3 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Role</th>
              <th className="px-3 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium text-right">Stake · ETH</th>
              <th className="px-3 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium text-right">Signed / Missed</th>
              <th className="px-3 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Trend · 1h</th>
              <th className="px-5 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium text-right">Last seen</th>
            </tr>
          </thead>
          <tbody>
            {OPERATORS.map((op, i) => {
              const total = op.signed + op.missed;
              const rate = total > 0 ? ((op.signed / total) * 100) : 0;
              const tone = rate > 99.5 ? 'success' : rate > 98 ? 'warning' : 'danger';
              return (
                <tr key={op.peerId} className={i > 0 ? 'border-t border-ink-border hover:bg-ink-surface-raised transition-colors duration-ds-fast' : 'hover:bg-ink-surface-raised transition-colors duration-ds-fast'}>
                  <td className="px-5 py-3 align-middle">
                    <div className="flex items-center gap-3">
                      <OperatorAvatar label={op.label} />
                      <div className="flex flex-col gap-0.5 min-w-0">
                        <span className="text-sm text-ink-fg leading-tight">{op.label}</span>
                        <Address value={op.peerId} truncate={6} />
                      </div>
                    </div>
                  </td>
                  <td className="px-3 py-3 align-middle">
                    {op.role === 'lead'
                      ? <Tag tone="accent" uppercase mono>Lead</Tag>
                      : <Tag tone="neutral" uppercase mono>Member</Tag>}
                  </td>
                  <td className="px-3 py-3 align-middle text-right">
                    <span className="font-mono text-sm text-ink-fg tabular-nums">{op.stake}</span>
                  </td>
                  <td className="px-3 py-3 align-middle text-right">
                    <div className="flex items-baseline justify-end gap-1.5 font-mono text-sm tabular-nums">
                      <span className="text-ink-fg">{op.signed.toLocaleString()}</span>
                      <span className="text-ink-fg-faint">/</span>
                      <span className={tone === 'success' ? 'text-ink-fg-secondary' : tone === 'warning' ? 'text-ink-warning' : 'text-ink-danger'}>{op.missed}</span>
                    </div>
                  </td>
                  <td className="px-3 py-3 align-middle">
                    <Sparkline values={op.trend} tone={tone === 'success' ? 'success' : tone === 'danger' ? 'danger' : 'accent'} />
                  </td>
                  <td className="px-5 py-3 align-middle text-right">
                    <span className="inline-flex items-center gap-2 justify-end">
                      <Status tone={staleness(op.lastSeenSec)} label="" />
                      <span className="font-mono text-xs text-ink-fg-secondary tabular-nums">{relativeTime(op.lastSeenSec)}</span>
                    </span>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </Surface>

      {/* Operator detail card + Quorum visualization */}
      <div className="grid grid-cols-[minmax(0,2fr)_minmax(0,1fr)] gap-5">
        <Surface variant="flat" className="p-6 flex flex-col gap-5">
          <div className="flex items-start justify-between gap-4">
            <div className="flex items-center gap-3">
              <OperatorAvatar label="coinbase-cloud" size="lg" />
              <div className="flex flex-col gap-1">
                <div className="flex items-center gap-2">
                  <span className="text-md text-ink-fg">coinbase-cloud</span>
                  <Tag tone="accent" uppercase mono>Lead</Tag>
                </div>
                <Address value="12D3KooWQYhDdR9k4n5K8z2vYcL7p3qA6mWxBfTuEsHj1cR2dF8x" truncate={8} />
              </div>
            </div>
            <Btn size="sm" variant="secondary">View on explorer</Btn>
          </div>
          <Divider />
          <div className="grid grid-cols-3 gap-6">
            <Metric label="Stake"          value="4,200" unit="ETH" size="sm" />
            <Metric label="Signing rate"   value="99.97" unit="%"   size="sm" delta={{ value: '0.02', direction: 'up' }} />
            <Metric label="Uptime · 30d"   value="99.99" unit="%"   size="sm" />
          </div>
          <div>
            <div className="flex items-center justify-between mb-2">
              <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Signed per 5-min · last 1h</span>
              <span className="font-mono text-xs text-ink-fg-faint">▼ 0 missed</span>
            </div>
            <SparkBars values={[58, 62, 60, 64, 59, 61, 63, 60, 62, 65, 61, 60]} />
          </div>
        </Surface>

        <Surface variant="flat" className="p-6 flex flex-col gap-4">
          <div className="flex items-baseline justify-between">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Quorum</span>
            <span className="font-mono text-sm text-ink-fg tabular-nums">14<span className="text-ink-fg-faint"> / 14</span></span>
          </div>
          <p className="text-xs text-ink-fg-secondary leading-relaxed">
            Threshold met for block <span className="font-mono text-ink-fg">19,847,221</span>. Quorum is the count of operators whose signature was included in the latest aggregation.
          </p>
          <div className="grid grid-cols-7 gap-1.5 mt-1">
            {Array.from({ length: 14 }).map((_, i) => {
              const tone = i === 12 ? 'pending' : i === 13 ? 'live' : 'live';
              const colorClass = tone === 'pending' ? 'bg-ink-warning' : 'bg-ink-success';
              return <span key={i} className={`h-3 rounded-ds-xs ${colorClass}`} />;
            })}
          </div>
          <div className="flex items-center gap-3 text-xs">
            <span className="inline-flex items-center gap-1.5 text-ink-fg-secondary">
              <span className="h-2 w-2 rounded-ds-xs bg-ink-success" /> signed
            </span>
            <span className="inline-flex items-center gap-1.5 text-ink-fg-secondary">
              <span className="h-2 w-2 rounded-ds-xs bg-ink-warning" /> pending
            </span>
            <span className="inline-flex items-center gap-1.5 text-ink-fg-secondary">
              <span className="h-2 w-2 rounded-ds-xs bg-ink-fg-faint" /> missed
            </span>
          </div>
        </Surface>
      </div>
    </div>
  );
}

function OperatorAvatar({ label, size = 'sm' }: { label: string; size?: 'sm' | 'lg' }) {
  const initials = label
    .split(/[-_\s]/)
    .map((p) => p[0])
    .filter(Boolean)
    .slice(0, 2)
    .join('')
    .toUpperCase();
  const dim = size === 'lg' ? 'h-10 w-10 text-sm' : 'h-7 w-7 text-xs';
  return (
    <div className={`shrink-0 ${dim} rounded-ds-xs bg-ink-surface-raised border border-ink-border flex items-center justify-center`}>
      <span className="font-mono text-ink-fg-secondary tracking-tight">{initials}</span>
    </div>
  );
}

function SparkBars({ values }: { values: number[] }) {
  const max = Math.max(...values, 1);
  return (
    <div className="flex items-end gap-1 h-12">
      {values.map((v, i) => (
        <span
          key={i}
          className="flex-1 bg-ink-accent-tint border-t border-ink-accent-edge rounded-ds-xs"
          style={{ height: `${(v / max) * 100}%` }}
        />
      ))}
    </div>
  );
}

/* ─── Domain — Components ──────────────────────────────────────── */

interface ComponentRow {
  name: string;
  digest: string;
  language: 'Rust' | 'AssemblyScript' | 'Go';
  size: string;
  usedBy: number;
  status: 'verified' | 'unverified';
}

const WASM_COMPONENTS: ComponentRow[] = [
  { name: 'oracle-twap',        digest: 'sha256:a78bfa6fc4b09b7dde2a1c0f9b3e6d05c8d7e2f4a1b2c3d4e5f6a7b8c9d0e1f2a3', language: 'Rust',           size: '1.2 MB', usedBy: 3, status: 'verified' },
  { name: 'sig-aggregator',     digest: 'sha256:d52f3a91b7c4e2d6f8a1b9c0e3d5f7a2b4c6d8e0f1a2b3c4d5e6f7a8b9c0d1e2f', language: 'Rust',           size: '412 KB', usedBy: 8, status: 'verified' },
  { name: 'attestation-verify', digest: 'sha256:f8e1b2a3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a', language: 'Rust',           size: '940 KB', usedBy: 4, status: 'verified' },
  { name: 'risk-engine',        digest: 'sha256:c91a7e2d4f6b8a0c1e3d5f7b9a2c4e6f8a1b3c5d7e9f0a2b4c6d8e0f1a3b5c7d9', language: 'AssemblyScript', size: '2.4 MB', usedBy: 2, status: 'verified' },
  { name: 'btc-relay',          digest: 'sha256:b34e2d5f7a9c1e3b5d7f9a1c3e5d7f9a1b3c5d7e9f1a3b5c7d9e1f3a5b7c9d1e3', language: 'Rust',           size: '856 KB', usedBy: 1, status: 'verified' },
  { name: 'bridge-prover',      digest: 'sha256:e23c5d7f9a1b3c5d7e9f1a3b5c7d9e1f3a5b7c9d1e3f5a7b9c1d3e5f7a9b1c3d5', language: 'Go',             size: '1.8 MB', usedBy: 2, status: 'unverified' },
];

function ComponentsPattern() {
  return (
    <div className="flex flex-col gap-6">
      {/* Filter bar + grid */}
      <div className="flex items-center gap-3">
        <Input leading={<SearchIcon />} placeholder="Search by name or digest…" className="flex-1" />
        <Tabs
          variant="segmented"
          active="all"
          onChange={() => undefined}
          items={[
            { key: 'all',         label: 'All',          badge: <Tag tone="neutral" mono>6</Tag> },
            { key: 'verified',    label: 'Verified',     badge: <Tag tone="success" mono>5</Tag> },
            { key: 'unverified',  label: 'Unverified',   badge: <Tag tone="warning" mono>1</Tag> },
          ]}
        />
        <Btn size="sm" variant="primary" leading={<PlusIcon />}>Upload</Btn>
      </div>

      <div className="grid grid-cols-2 gap-3">
        {WASM_COMPONENTS.map((c) => (
          <Surface key={c.digest} variant="flat" className="p-5 flex flex-col gap-3 hover:bg-ink-surface-raised transition-colors duration-ds-fast cursor-pointer group">
            <div className="flex items-start justify-between gap-3">
              <div className="flex flex-col gap-1 min-w-0">
                <div className="flex items-center gap-2">
                  <span className="text-sm text-ink-fg group-hover:text-ink-accent transition-colors duration-ds-fast">{c.name}</span>
                  {c.status === 'verified'
                    ? <Tag tone="success" uppercase mono leading={<TickIcon />}>Verified</Tag>
                    : <Tag tone="warning" uppercase mono>Unverified</Tag>}
                </div>
                <Address value={c.digest} truncate={8} />
              </div>
              <Tag tone="neutral" mono>{c.language}</Tag>
            </div>
            <Divider />
            <div className="flex items-center justify-between text-xs">
              <span className="font-mono text-ink-fg-muted">{c.size}</span>
              <span className="text-ink-fg-secondary">
                Used by <span className="font-mono text-ink-fg">{c.usedBy}</span> service{c.usedBy === 1 ? '' : 's'}
              </span>
            </div>
          </Surface>
        ))}
      </div>

      {/* Component detail */}
      <Surface variant="flat" className="overflow-hidden">
        <div className="flex items-start justify-between gap-4 px-6 py-4 border-b border-ink-border">
          <div className="flex flex-col gap-1.5">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Component</span>
            <div className="flex items-baseline gap-3">
              <span className="text-lg text-ink-fg">oracle-twap</span>
              <Tag tone="success" uppercase mono leading={<TickIcon />}>Verified</Tag>
              <Tag tone="neutral" mono>Rust 1.91 · WASI 0.2</Tag>
            </div>
            <Address value="sha256:a78bfa6fc4b09b7dde2a1c0f9b3e6d05c8d7e2f4a1b2c3d4e5f6a7b8c9d0e1f2a3" truncate={false} />
          </div>
          <div className="flex items-center gap-2">
            <Btn size="sm" variant="ghost" leading={<DownloadIcon />}>Download</Btn>
            <Btn size="sm" variant="primary">Deploy as service</Btn>
          </div>
        </div>
        <div className="grid grid-cols-3 divide-x divide-ink-border">
          <div className="p-5 flex flex-col gap-4">
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Manifest</div>
            <Stat label="Size"            value="1,283,712 bytes" />
            <Stat label="Imports"         value="8" />
            <Stat label="Exports"         value="run, configure" />
            <Stat label="Memory · max"    value="64 MB" />
            <Stat label="Stack · max"     value="1 MB" />
          </div>
          <div className="p-5 flex flex-col gap-4">
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Runtime</div>
            <Stat label="Engine"          value="Wasmtime 24.0" />
            <Stat label="Determinism"     value={<><span className="text-ink-success">guaranteed</span></>} mono={false} />
            <Stat label="Network"         value={<>http · <span className="text-ink-fg-faint">no fs</span></>} mono={false} />
            <Stat label="Avg. exec time"  value="142 ms" />
            <Stat label="P99 exec time"   value="318 ms" />
          </div>
          <div className="p-5 flex flex-col gap-3">
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Used by</div>
            {[
              { name: 'price-oracle-mainnet', tone: 'live' as const },
              { name: 'price-oracle-sepolia', tone: 'live' as const },
              { name: 'twap-aggregator',      tone: 'paused' as const },
            ].map((s) => (
              <div key={s.name} className="flex items-center justify-between text-sm py-1.5 border-b border-ink-border last:border-b-0">
                <span className="text-ink-fg">{s.name}</span>
                <Status tone={s.tone} />
              </div>
            ))}
          </div>
        </div>
      </Surface>
    </div>
  );
}

/* ─── Domain — Services ────────────────────────────────────────── */

interface ServiceRow {
  name: string;
  status: 'live' | 'pending' | 'paused' | 'error';
  chain: string;
  triggersHr: string;
  latencyMs: number;
  operators: string;
  manager: string;
}

const SERVICES: ServiceRow[] = [
  { name: 'price-oracle-mainnet',     status: 'live',    chain: 'Ethereum',  triggersHr: '1,284', latencyMs: 142, operators: '14 / 14', manager: '0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80' },
  { name: 'attestation-relay',        status: 'live',    chain: 'Ethereum',  triggersHr: '342',   latencyMs: 198, operators: '14 / 14', manager: '0x91b9d3a4Cc6634C0532925a3b844Bc9e7595fAcEf' },
  { name: 'slashing-monitor',         status: 'live',    chain: 'Ethereum',  triggersHr: '12',    latencyMs: 84,  operators: '13 / 14', manager: '0xa3c8d2bE6634C0532925a3b844Bc9e7595fB3D29' },
  { name: 'twap-aggregator',          status: 'pending', chain: 'Sepolia',   triggersHr: '0',     latencyMs: 0,   operators: '0 / 14',  manager: '0xbf7d12cE6634C0532925a3b844Bc9e7595f8C04A' },
  { name: 'bridge-validator-sepolia', status: 'paused',  chain: 'Sepolia',   triggersHr: '0',     latencyMs: 0,   operators: '8 / 14',  manager: '0xeb2a9f4D6634C0532925a3b844Bc9e7595f3Aa12' },
];

function ServicesPattern() {
  return (
    <div className="flex flex-col gap-6">
      {/* Service catalog list */}
      <Surface variant="flat" className="overflow-hidden">
        <div className="flex items-center justify-between px-5 py-3 border-b border-ink-border">
          <div className="flex items-center gap-3">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Catalog</span>
            <Tag tone="success" mono leading={<DotIcon />}>3 live</Tag>
            <Tag tone="warning" mono>1 pending</Tag>
            <Tag tone="neutral" mono>1 paused</Tag>
          </div>
          <Btn size="sm" variant="primary" leading={<PlusIcon />}>Deploy service</Btn>
        </div>
        {SERVICES.map((s, i) => (
          <div key={s.name} className={`px-5 py-4 grid grid-cols-[minmax(0,2fr)_repeat(4,minmax(0,1fr))_auto] items-center gap-4 hover:bg-ink-surface-raised transition-colors duration-ds-fast cursor-pointer ${i > 0 ? 'border-t border-ink-border' : ''}`}>
            <div className="flex flex-col gap-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm text-ink-fg">{s.name}</span>
                <Status tone={s.status} />
              </div>
              <div className="flex items-center gap-2">
                <Tag tone="neutral" mono>{s.chain}</Tag>
                <Address value={s.manager} truncate={4} />
              </div>
            </div>
            <Stat label="Triggers · 1h" value={s.triggersHr} />
            <Stat label="P50 latency"   value={s.latencyMs > 0 ? `${s.latencyMs}ms` : '—'} />
            <Stat label="Operators"     value={s.operators} />
            <Stat label="Component"     value="oracle-twap" mono={false} />
            <ArrowIcon />
          </div>
        ))}
      </Surface>

      {/* Service detail header */}
      <Surface variant="flat" className="overflow-hidden">
        {/* Breadcrumb */}
        <div className="flex items-center gap-2 px-6 pt-4 text-xs">
          <span className="text-ink-fg-muted">services</span>
          <span className="text-ink-fg-faint">/</span>
          <span className="text-ink-fg-muted">ethereum</span>
          <span className="text-ink-fg-faint">/</span>
          <span className="text-ink-fg">price-oracle-mainnet</span>
        </div>

        {/* Hero */}
        <div className="flex items-start justify-between gap-6 px-6 pt-3 pb-5">
          <div className="flex flex-col gap-2 min-w-0">
            <div className="flex items-center gap-3">
              <h3 className="text-xl text-ink-fg">price-oracle-mainnet</h3>
              <Status tone="live" />
              <Tag tone="accent" mono uppercase>v0.4.2</Tag>
            </div>
            <Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" />
          </div>
          <div className="flex items-center gap-2">
            <Btn size="sm" variant="ghost" leading={<RefreshIcon />}>Replay</Btn>
            <Btn size="sm" variant="secondary">Edit configuration</Btn>
            <Btn size="sm" variant="danger" leading={<PauseIcon />}>Pause</Btn>
          </div>
        </div>

        {/* KPI row */}
        <div className="grid grid-cols-5 divide-x divide-ink-border border-y border-ink-border">
          <Metric className="p-5" label="Triggers · 1h"    value="1,284"     size="sm" delta={{ value: '4%', direction: 'up' }} />
          <Metric className="p-5" label="P50 latency"      value="142" unit="ms" size="sm" delta={{ value: '12 ms', direction: 'down' }} />
          <Metric className="p-5" label="P99 latency"      value="318" unit="ms" size="sm" />
          <Metric className="p-5" label="Operators"        value="14 / 14"   size="sm" />
          <Metric className="p-5" label="Failed runs · 24h" value="0"         size="sm" delta={{ value: '0', direction: 'flat' }} />
        </div>

        {/* Sub-tabs */}
        <div className="px-6 pt-2">
          <Tabs
            active="overview"
            onChange={() => undefined}
            items={[
              { key: 'overview',    label: 'Overview' },
              { key: 'triggers',    label: 'Triggers',    badge: <Tag tone="accent" mono>12</Tag> },
              { key: 'submissions', label: 'Submissions', badge: <Tag tone="warning" mono>2</Tag> },
              { key: 'config',      label: 'Configuration' },
              { key: 'logs',        label: 'Logs' },
            ]}
          />
        </div>

        {/* Tab body — overview */}
        <div className="grid grid-cols-2 gap-6 px-6 py-6">
          <div className="flex flex-col gap-3">
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Configuration</div>
            <Stat label="Chain"           value="Ethereum mainnet" />
            <Stat label="Service manager" value={<Address value="0x742d35Cc6634C0532925a3b844Bc9e7595f2bD80" />} mono={false} />
            <Stat label="Component"       value="oracle-twap" mono={false} />
            <Stat label="Component digest" value={<Address value="sha256:a78bfa6fc4b09b7dde2a1c0f9b3e6d05" truncate={6} />} mono={false} />
            <Stat label="Trigger source"  value="block · every 12s" />
            <Stat label="Aggregator"      value="enabled · 2/3 threshold" />
          </div>
          <div className="flex flex-col gap-3">
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Triggers · last 1h</div>
            <SparkBars values={[58, 62, 60, 64, 59, 61, 63, 60, 62, 65, 61, 60]} />
            <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mt-3">Latency · ms · last 1h</div>
            <SparkBars values={[140, 138, 142, 145, 139, 141, 144, 142, 140, 143, 145, 141]} />
          </div>
        </div>
      </Surface>
    </div>
  );
}

/* ─── Domain — Events ──────────────────────────────────────────── */

type EventKind = 'trigger' | 'execute' | 'sign' | 'submit' | 'peer' | 'error';

interface EventRow {
  ts: string;
  kind: EventKind;
  service: string;
  body: string;
  meta?: string;
}

const EVENT_LIBRARY: EventRow[] = [
  { ts: '14:22:08.142', kind: 'trigger', service: 'price-oracle-mainnet', body: 'block 19,847,221 received', meta: 'lag=12ms' },
  { ts: '14:22:08.183', kind: 'execute', service: 'price-oracle-mainnet', body: 'oracle-twap@a78bfa6f executed', meta: 'duration=142ms' },
  { ts: '14:22:08.221', kind: 'sign',    service: 'price-oracle-mainnet', body: 'signature collected from 12D3KooWQYhDdR…', meta: '14/14' },
  { ts: '14:22:08.298', kind: 'submit',  service: 'price-oracle-mainnet', body: 'submission tx 0xab12…cd34', meta: 'gas=84,212' },
  { ts: '14:22:09.014', kind: 'trigger', service: 'attestation-relay',    body: 'attestation request received',           meta: '' },
  { ts: '14:22:09.211', kind: 'execute', service: 'attestation-relay',    body: 'attestation-verify@f8e1b2a3 executed',   meta: 'duration=198ms' },
  { ts: '14:22:09.418', kind: 'peer',    service: 'system',                body: 'peer 12D3KooW…2vYcL7 reconnected',     meta: '' },
  { ts: '14:22:10.822', kind: 'error',   service: 'twap-aggregator',     body: 'execution failed: WASI trap "unreachable"', meta: 'retry in 8s' },
];

function eventToneFor(k: EventKind): 'accent' | 'success' | 'info' | 'warning' | 'danger' | 'neutral' {
  switch (k) {
    case 'trigger': return 'info';
    case 'execute': return 'accent';
    case 'sign':    return 'success';
    case 'submit':  return 'success';
    case 'peer':    return 'neutral';
    case 'error':   return 'danger';
  }
}

function EventsPattern() {
  return (
    <div className="flex flex-col gap-6">
      <LiveTail />

      {/* Activity table */}
      <Surface variant="flat" className="overflow-hidden">
        <div className="flex items-center justify-between gap-4 px-5 py-3 border-b border-ink-border">
          <div className="flex items-center gap-2">
            <Input leading={<SearchIcon />} placeholder="Search events" className="w-64" />
            <Tabs
              variant="segmented"
              active="all"
              onChange={() => undefined}
              items={[
                { key: 'all',     label: 'All' },
                { key: 'trigger', label: 'Triggers' },
                { key: 'execute', label: 'Executions' },
                { key: 'submit',  label: 'Submissions' },
                { key: 'error',   label: 'Errors',     badge: <Tag tone="danger" mono>1</Tag> },
              ]}
            />
          </div>
          <div className="flex items-center gap-3">
            <span className="font-mono text-xs text-ink-fg-muted">last 1h</span>
            <Btn size="sm" variant="ghost" leading={<DownloadIcon />}>Export</Btn>
          </div>
        </div>
        <table className="w-full">
          <thead>
            <tr className="text-left bg-ink-surface-sunken border-b border-ink-border">
              <th className="px-5 py-2.5 w-40 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Timestamp</th>
              <th className="px-3 py-2.5 w-28 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Type</th>
              <th className="px-3 py-2.5 w-56 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Service</th>
              <th className="px-3 py-2.5 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Detail</th>
              <th className="px-5 py-2.5 w-32 font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium text-right">Meta</th>
            </tr>
          </thead>
          <tbody>
            {EVENT_LIBRARY.map((e, i) => (
              <tr key={i} className={`hover:bg-ink-surface-raised transition-colors duration-ds-fast ${i > 0 ? 'border-t border-ink-border' : ''}`}>
                <td className="px-5 py-2.5">
                  <span className="font-mono text-xs text-ink-fg-secondary tabular-nums">2026-04-28 {e.ts}</span>
                </td>
                <td className="px-3 py-2.5">
                  <Tag tone={eventToneFor(e.kind)} mono uppercase>{e.kind}</Tag>
                </td>
                <td className="px-3 py-2.5">
                  <span className="text-sm text-ink-fg">{e.service}</span>
                </td>
                <td className="px-3 py-2.5">
                  <span className="font-mono text-xs text-ink-fg-secondary">{e.body}</span>
                </td>
                <td className="px-5 py-2.5 text-right">
                  <span className="font-mono text-xs text-ink-fg-faint">{e.meta}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="flex items-center justify-between px-5 py-3 border-t border-ink-border">
          <span className="font-mono text-xs text-ink-fg-muted">Showing 8 of 4,210 · paginated</span>
          <div className="flex items-center gap-2">
            <Btn size="sm" variant="ghost">← Prev</Btn>
            <span className="font-mono text-xs text-ink-fg-secondary">page 1 / 527</span>
            <Btn size="sm" variant="ghost">Next →</Btn>
          </div>
        </div>
      </Surface>

      {/* Volume / time-series */}
      <Surface variant="flat" className="p-6 flex flex-col gap-4">
        <div className="flex items-baseline justify-between">
          <div className="flex items-baseline gap-3">
            <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Trigger volume</span>
            <span className="font-mono text-sm text-ink-fg tabular-nums">4,210</span>
            <span className="font-mono text-xs text-ink-fg-secondary">events · last 1h</span>
            <Tag tone="success" mono leading={<ArrowUpIcon />}>+4.2%</Tag>
          </div>
          <div className="flex items-center gap-1">
            {(['1h', '6h', '24h', '7d'] as const).map((r, i) => (
              <span key={r} className={`font-mono text-xs px-2 h-6 inline-flex items-center rounded-ds-xs cursor-pointer ${i === 0 ? 'bg-ink-surface-raised text-ink-fg' : 'text-ink-fg-muted hover:text-ink-fg'}`}>{r}</span>
            ))}
          </div>
        </div>
        <SparkBars values={[40, 52, 48, 60, 58, 64, 70, 68, 62, 75, 80, 78, 82, 76, 84, 88, 86, 92, 88, 90, 94, 91, 96, 100]} />
        <div className="flex items-center justify-between font-mono text-xs text-ink-fg-faint">
          <span>13:22</span>
          <span>14:22</span>
        </div>
      </Surface>
    </div>
  );
}

function LiveTail() {
  const [paused, setPaused] = useState(false);
  const [tick, setTick] = useState(0);

  // Cycle through events to simulate a live tail
  useEffect(() => {
    if (paused) return;
    const id = window.setInterval(() => setTick((t) => t + 1), 1400);
    return () => window.clearInterval(id);
  }, [paused]);

  const visible = EVENT_LIBRARY.slice(0, 6).map((e, i) => ({
    ...e,
    pulse: !paused && i === tick % 6,
  }));

  return (
    <Surface variant="flat" className="overflow-hidden">
      <div className="flex items-center justify-between gap-4 px-5 py-3 border-b border-ink-border">
        <div className="flex items-center gap-3">
          <Status tone={paused ? 'paused' : 'live'} label={paused ? 'Paused' : 'Tailing'} />
          <span className="font-mono text-xs text-ink-fg-muted">Live event stream · all services</span>
        </div>
        <div className="flex items-center gap-2">
          <Btn size="sm" variant="ghost" onClick={() => setPaused((p) => !p)} leading={paused ? <PlayIcon /> : <PauseIcon />}>
            {paused ? 'Resume' : 'Pause'}
          </Btn>
          <Btn size="sm" variant="ghost">Clear</Btn>
        </div>
      </div>
      <div className="bg-ink-surface-sunken">
        <div className="flex flex-col">
          {visible.map((e, i) => (
            <div
              key={`${e.ts}-${i}`}
              className={`grid grid-cols-[140px_90px_1fr_auto] items-baseline gap-3 px-5 py-2 border-b border-ink-border last:border-b-0 transition-colors duration-ds-base ${e.pulse ? 'bg-ink-accent-tint' : ''}`}
            >
              <span className="font-mono text-xs text-ink-fg-faint tabular-nums">{e.ts}</span>
              <Tag tone={eventToneFor(e.kind)} mono uppercase>{e.kind}</Tag>
              <span className="font-mono text-xs text-ink-fg-secondary truncate">
                <span className="text-ink-fg">{e.service}</span>
                <span className="text-ink-fg-faint"> · </span>
                {e.body}
              </span>
              <span className="font-mono text-xs text-ink-fg-faint">{e.meta}</span>
            </div>
          ))}
        </div>
      </div>
      <div className="flex items-center justify-between px-5 py-2 border-t border-ink-border">
        <span className="font-mono text-xs text-ink-fg-muted">Auto-scroll {paused ? 'off' : 'on'}</span>
        <span className="font-mono text-xs text-ink-fg-faint">scroll up to pause · ⌘↩ to clear</span>
      </div>
    </Surface>
  );
}

/* ─── Domain — Logs ────────────────────────────────────────────── */

type LogLvl = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'TRACE';

interface LogLine {
  ts: string;
  lvl: LogLvl;
  target: string;
  msg: string;
  fields?: Record<string, string>;
  stack?: string[];
}

const LOG_LINES: LogLine[] = [
  { ts: '14:22:08.142', lvl: 'INFO',  target: 'wavs::trigger',     msg: 'received block trigger',          fields: { service: 'price-oracle-mainnet', block: '19847221', lag_ms: '12' } },
  { ts: '14:22:08.183', lvl: 'DEBUG', target: 'wavs::engine',       msg: 'wasm component invoked',          fields: { digest: 'a78bfa6f', memory_kb: '32768' } },
  { ts: '14:22:08.221', lvl: 'INFO',  target: 'wavs::aggregator',   msg: 'signature collected',             fields: { peer: '12D3KooW…', threshold: '14/14' } },
  { ts: '14:22:08.298', lvl: 'INFO',  target: 'wavs::submission',   msg: 'evm submission confirmed',        fields: { tx: '0xab12…cd34', gas: '84212' } },
  { ts: '14:22:09.014', lvl: 'INFO',  target: 'wavs::trigger',      msg: 'received attestation request',    fields: { service: 'attestation-relay' } },
  { ts: '14:22:09.211', lvl: 'INFO',  target: 'wavs::engine',       msg: 'execution complete',              fields: { duration_ms: '198', exit_code: '0' } },
  { ts: '14:22:09.418', lvl: 'TRACE', target: 'wavs::p2p',          msg: 'gossipsub message received',      fields: { topic: '/wavs/aggregate/1', size: '418' } },
  { ts: '14:22:10.006', lvl: 'WARN',  target: 'wavs::aggregator',   msg: 'peer signature arrived after deadline', fields: { peer: '12D3KooW…2vYcL7', late_ms: '1240' } },
  { ts: '14:22:10.418', lvl: 'DEBUG', target: 'wavs::engine',       msg: 'compiled module cache hit',       fields: { digest: 'd52f3a91' } },
  { ts: '14:22:10.822', lvl: 'ERROR', target: 'wavs::engine',       msg: 'execution failed: WASI trap',
    fields: { service: 'twap-aggregator', digest: 'c91a7e2d', kind: 'unreachable' },
    stack: [
      'at component::oracle::compute (component.wasm:0x1a4f)',
      'at component::oracle::run (component.wasm:0x0b22)',
      'at __wasi_export_run (component.wasm:0x0044)',
    ],
  },
];

const LEVEL_COLOR: Record<LogLvl, string> = {
  ERROR: 'text-ink-danger',
  WARN:  'text-ink-warning',
  INFO:  'text-ink-info',
  DEBUG: 'text-ink-accent',
  TRACE: 'text-ink-fg-muted',
};

const LEVEL_COUNTS: Record<LogLvl, number> = {
  ERROR: 1, WARN: 1, INFO: 4, DEBUG: 2, TRACE: 1,
};

function LogsPattern() {
  return (
    <div className="flex flex-col gap-6">
      <FullLogViewer />

      {/* Compact embedded log + Anatomy */}
      <div className="grid grid-cols-2 gap-5">
        <CompactLogEmbed />
        <LogRowAnatomy />
      </div>

      {/* Error with expanded detail */}
      <ExpandedErrorLog />
    </div>
  );
}

function FullLogViewer() {
  const [active, setActive] = useState<Set<LogLvl>>(new Set(['ERROR', 'WARN', 'INFO']));
  const [follow, setFollow] = useState(true);
  const [search, setSearch] = useState('');
  const [tick, setTick] = useState(0);

  useEffect(() => {
    if (!follow) return;
    const id = window.setInterval(() => setTick((t) => t + 1), 1800);
    return () => window.clearInterval(id);
  }, [follow]);

  const toggleLevel = (l: LogLvl) => {
    setActive((prev) => {
      const next = new Set(prev);
      if (next.has(l)) next.delete(l);
      else next.add(l);
      return next;
    });
  };

  const visible = LOG_LINES
    .filter((l) => active.has(l.lvl))
    .filter((l) => !search || l.msg.toLowerCase().includes(search.toLowerCase()) || l.target.includes(search));

  return (
    <Surface variant="flat" className="overflow-hidden">
      {/* Header bar */}
      <div className="flex items-center justify-between gap-4 px-5 py-3 border-b border-ink-border">
        <div className="flex items-center gap-2">
          {(['ERROR', 'WARN', 'INFO', 'DEBUG', 'TRACE'] as LogLvl[]).map((lvl) => {
            const isOn = active.has(lvl);
            return (
              <button
                key={lvl}
                type="button"
                onClick={() => toggleLevel(lvl)}
                className={`inline-flex items-center gap-1.5 h-6 px-2 rounded-ds-xs font-mono text-xs uppercase tracking-widest border transition-colors duration-ds-fast cursor-pointer ${
                  isOn
                    ? `${LEVEL_COLOR[lvl]} bg-ink-surface-raised border-ink-border-strong`
                    : 'text-ink-fg-faint border-ink-border bg-transparent hover:text-ink-fg-muted'
                }`}
              >
                <span>{lvl}</span>
                <span className="text-ink-fg-faint">{LEVEL_COUNTS[lvl]}</span>
              </button>
            );
          })}
        </div>
        <div className="flex items-center gap-2">
          <Input
            leading={<SearchIcon />}
            placeholder="Search messages or target…"
            value={search}
            onChange={setSearch}
            className="w-64"
          />
          <Btn size="sm" variant="ghost" leading={<DownloadIcon />}>Export</Btn>
          <Btn size="sm" variant="ghost">Clear</Btn>
        </div>
      </div>

      {/* Source context bar */}
      <div className="flex items-center justify-between px-5 py-2 bg-ink-surface-sunken border-b border-ink-border text-xs">
        <div className="flex items-center gap-3 font-mono text-ink-fg-muted">
          <span>source: <span className="text-ink-fg-secondary">all targets</span></span>
          <span className="text-ink-fg-faint">·</span>
          <span>showing <span className="text-ink-fg">{visible.length}</span> of {LOG_LINES.length} lines</span>
          <span className="text-ink-fg-faint">·</span>
          <span>{follow ? <span className="text-ink-success">following tail</span> : <span className="text-ink-warning">scrolled · paused</span>}</span>
        </div>
        <Btn size="sm" variant="ghost" onClick={() => setFollow((f) => !f)} leading={follow ? <PauseIcon /> : <PlayIcon />}>
          {follow ? 'Stop following' : 'Follow tail'}
        </Btn>
      </div>

      {/* Log rows */}
      <div className="bg-ink-surface-sunken">
        {visible.map((l, i) => {
          const pulse = follow && i === visible.length - 1 && tick > 0;
          return <LogRow key={`${l.ts}-${i}-${tick}`} line={l} pulse={pulse} />;
        })}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between px-5 py-2 border-t border-ink-border">
        <span className="font-mono text-xs text-ink-fg-muted">
          ⌘F to search · ⌘L to clear · ⌘D toggle DEBUG
        </span>
        {!follow && (
          <Btn size="sm" variant="primary" onClick={() => setFollow(true)} leading={<ArrowDownIcon />}>
            Jump to live
          </Btn>
        )}
      </div>
    </Surface>
  );
}

function LogRow({ line, pulse }: { line: LogLine; pulse?: boolean }) {
  const isAlert = line.lvl === 'ERROR' || line.lvl === 'WARN';
  return (
    <div className={[
      'grid grid-cols-[140px_70px_1fr] gap-3 px-5 py-2 border-b border-ink-border last:border-b-0',
      'transition-colors duration-ds-base',
      isAlert ? 'bg-ink-danger-tint/40' : '',
      pulse ? 'bg-ink-accent-tint' : '',
      'hover:bg-ink-surface-raised',
    ].join(' ')}>
      <span className="font-mono text-xs text-ink-fg-faint tabular-nums leading-relaxed">{line.ts}</span>
      <span className={`font-mono text-xs uppercase tracking-widest leading-relaxed ${LEVEL_COLOR[line.lvl]}`}>{line.lvl}</span>
      <div className="flex flex-col gap-1 min-w-0">
        <div className="flex items-baseline gap-2 flex-wrap">
          <span className="font-mono text-xs text-ink-fg-muted">{line.target}</span>
          <span className="font-mono text-xs text-ink-fg leading-relaxed">{line.msg}</span>
        </div>
        {line.fields && (
          <div className="flex items-center gap-2 flex-wrap">
            {Object.entries(line.fields).map(([k, v]) => (
              <span key={k} className="font-mono text-xs">
                <span className="text-ink-fg-faint">{k}=</span>
                <span className="text-ink-fg-secondary">{v}</span>
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function CompactLogEmbed() {
  return (
    <Surface variant="flat" className="overflow-hidden">
      <div className="flex items-center justify-between px-4 py-2 border-b border-ink-border">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Compact · embedded</span>
        <Btn size="sm" variant="ghost">Open full view →</Btn>
      </div>
      <div className="bg-ink-surface-sunken">
        {LOG_LINES.slice(0, 5).map((l, i) => (
          <div key={i} className="flex items-baseline gap-2 px-4 py-1 border-b border-ink-border last:border-b-0 hover:bg-ink-surface-raised">
            <span className="font-mono text-[10px] text-ink-fg-faint tabular-nums shrink-0">{l.ts}</span>
            <span className={`font-mono text-[10px] uppercase shrink-0 ${LEVEL_COLOR[l.lvl]}`}>{l.lvl[0]}</span>
            <span className="font-mono text-xs text-ink-fg truncate">{l.msg}</span>
          </div>
        ))}
      </div>
      <div className="px-4 py-1.5 border-t border-ink-border text-center">
        <span className="font-mono text-[10px] text-ink-fg-muted">5 of 142 · service detail panel</span>
      </div>
    </Surface>
  );
}

function LogRowAnatomy() {
  return (
    <Surface variant="outline" className="p-5 flex flex-col gap-4">
      <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Row anatomy</span>
      <div className="rounded-ds-xs bg-ink-surface-sunken border border-ink-border p-3">
        <LogRow line={LOG_LINES[0]} />
      </div>
      <div className="grid grid-cols-[16px_1fr] gap-x-3 gap-y-2 text-xs">
        <span className="font-mono text-ink-accent">①</span>
        <p className="text-ink-fg-secondary"><span className="font-mono text-ink-fg">timestamp</span> — millisecond precision, monospace, tabular-nums for visual stability when streaming.</p>
        <span className="font-mono text-ink-accent">②</span>
        <p className="text-ink-fg-secondary"><span className="font-mono text-ink-fg">level</span> — fixed-width column. Color carries severity; row stays neutral except for ERROR/WARN.</p>
        <span className="font-mono text-ink-accent">③</span>
        <p className="text-ink-fg-secondary"><span className="font-mono text-ink-fg">target</span> — Rust crate path. Quiet, secondary color — for context, not foreground.</p>
        <span className="font-mono text-ink-accent">④</span>
        <p className="text-ink-fg-secondary"><span className="font-mono text-ink-fg">message</span> — primary content. The first thing the eye should land on.</p>
        <span className="font-mono text-ink-accent">⑤</span>
        <p className="text-ink-fg-secondary"><span className="font-mono text-ink-fg">fields</span> — key=value pairs, faint keys, secondary values. Wraps below message; never obscures it.</p>
      </div>
    </Surface>
  );
}

function ExpandedErrorLog() {
  const err = LOG_LINES[LOG_LINES.length - 1];
  return (
    <Surface variant="flat" className="overflow-hidden">
      <div className="flex items-center justify-between px-5 py-2 border-b border-ink-border bg-ink-danger-tint/30">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-danger">Error · expanded</span>
        <Tag tone="danger" mono uppercase>retry in 8s</Tag>
      </div>
      <div className="bg-ink-surface-sunken">
        <LogRow line={err} />
        <div className="px-5 py-3 border-t border-ink-border">
          <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-2">Stack trace</div>
          <pre className="font-mono text-xs text-ink-fg-secondary leading-relaxed">
            {err.stack?.map((line, i) => (
              <div key={i} className="flex gap-3 py-0.5">
                <span className="text-ink-fg-faint w-6 text-right tabular-nums">{i}</span>
                <span>{line}</span>
              </div>
            ))}
          </pre>
        </div>
        <div className="px-5 py-3 border-t border-ink-border flex items-center gap-2">
          <Btn size="sm" variant="secondary" leading={<RefreshIcon />}>Replay trigger</Btn>
          <Btn size="sm" variant="ghost">Open service</Btn>
          <Btn size="sm" variant="ghost">Filter to digest</Btn>
          <span className="ml-auto font-mono text-xs text-ink-fg-muted">⌘. opens this row</span>
        </div>
      </div>
    </Surface>
  );
}

/* ─── Feedback — Alerts ────────────────────────────────────────── */

function AlertsPattern() {
  const tones: NotifyTone[] = ['info', 'success', 'warning', 'danger', 'accent', 'neutral'];
  return (
    <div className="flex flex-col gap-6">
      {/* Inline alerts grid */}
      <div className="flex flex-col gap-3">
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Inline · contained</div>
        {tones.map((tone) => (
          <Alert
            key={tone}
            tone={tone}
            title={tone === 'danger' ? 'Submission rejected' : tone === 'warning' ? 'Aggregator restarting' : tone === 'success' ? 'Service deployed' : tone === 'info' ? 'New WAVS version available' : tone === 'accent' ? 'Component verified' : 'Operator note'}
            description={
              tone === 'danger'
                ? 'EVM transaction reverted: insufficient operator quorum (8/14). Threshold is 10.'
                : tone === 'warning'
                  ? 'P2P aggregation will resume in ~30 seconds. New triggers are queued, not lost.'
                  : tone === 'success'
                    ? 'price-oracle-mainnet is live and accepting triggers from block 19,847,221.'
                    : tone === 'info'
                      ? 'WAVS v0.5.1 includes a Wasmtime bump and 3× faster aggregator startup.'
                      : tone === 'accent'
                        ? 'Component digest matches the registry attestation. Safe to deploy.'
                        : 'This service has been running for 24 days with zero failed submissions.'
            }
            action={
              tone === 'danger' ? <Btn size="sm" variant="danger">Inspect tx</Btn>
              : tone === 'warning' ? <Btn size="sm" variant="ghost">View status</Btn>
              : tone === 'info' ? <Btn size="sm" variant="ghost">Release notes</Btn>
              : null
            }
            onDismiss={tone === 'success' || tone === 'accent' || tone === 'neutral' ? () => undefined : undefined}
          />
        ))}
      </div>

      {/* Banner */}
      <div className="flex flex-col gap-3">
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Banner · full width</div>
        <Surface variant="outline" className="overflow-hidden">
          <Alert
            variant="banner"
            tone="warning"
            title="Node out of sync"
            description="Local block height lags head by 124 blocks. Triggers are paused until the gap closes."
            action={<Btn size="sm" variant="secondary">Force resync</Btn>}
            onDismiss={() => undefined}
          />
          <div className="px-6 py-12 text-center">
            <span className="font-mono text-xs text-ink-fg-faint">— page content sits beneath the banner —</span>
          </div>
        </Surface>
      </div>

      {/* Stacked compound (multi-issue) */}
      <div className="flex flex-col gap-3">
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Compound · multi-issue</div>
        <Surface variant="outline" className="p-5 flex flex-col gap-3">
          <div className="flex items-baseline justify-between mb-1">
            <span className="text-sm text-ink-fg">3 issues need attention</span>
            <Btn size="sm" variant="ghost">Dismiss all</Btn>
          </div>
          <Alert tone="danger"  title="Operator p2p.org missed last 25 attestations" action={<Btn size="sm" variant="ghost">Open</Btn>} onDismiss={() => undefined} />
          <Alert tone="warning" title="Component bridge-prover is unverified"          action={<Btn size="sm" variant="ghost">Verify</Btn>} onDismiss={() => undefined} />
          <Alert tone="info"    title="Disk usage at 78% on this node"                  action={<Btn size="sm" variant="ghost">Manage</Btn>} onDismiss={() => undefined} />
        </Surface>
      </div>
    </div>
  );
}

/* ─── Feedback — Toasts ────────────────────────────────────────── */

interface ShowcaseToast {
  id: number;
  tone: NotifyTone;
  title: string;
  description?: string;
  action?: { label: string; onClick: () => void };
}

function ToastsPattern() {
  const [toasts, setToasts] = useState<ShowcaseToast[]>([]);
  const idRef = useRef(1);

  const push = (t: Omit<ShowcaseToast, 'id'>, autoMs = 5000) => {
    const id = idRef.current++;
    setToasts((prev) => [...prev, { ...t, id }]);
    if (autoMs > 0) {
      window.setTimeout(() => {
        setToasts((prev) => prev.filter((x) => x.id !== id));
      }, autoMs);
    }
  };

  const dismiss = (id: number) => setToasts((prev) => prev.filter((t) => t.id !== id));

  return (
    <div className="flex flex-col gap-6">
      <Surface variant="outline" className="p-6 flex flex-col gap-4">
        <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Trigger · click to push</div>
        <div className="flex flex-wrap gap-2">
          <Btn size="sm" variant="secondary" onClick={() => push({
            tone: 'success',
            title: 'Service deployed',
            description: 'price-oracle-mainnet is live.',
          })}>Success</Btn>

          <Btn size="sm" variant="secondary" onClick={() => push({
            tone: 'info',
            title: 'New version available',
            description: 'WAVS v0.5.1 — restart to apply.',
            action: { label: 'Restart', onClick: () => undefined },
          })}>Info + action</Btn>

          <Btn size="sm" variant="secondary" onClick={() => push({
            tone: 'warning',
            title: 'Aggregator pausing',
            description: 'Resuming in 30s.',
          })}>Warning</Btn>

          <Btn size="sm" variant="secondary" onClick={() => push({
            tone: 'danger',
            title: 'Submission failed',
            description: 'Tx 0xab12…cd34 reverted: insufficient quorum.',
            action: { label: 'Retry', onClick: () => undefined },
          }, 0)}>Error · sticky</Btn>

          <Btn size="sm" variant="ghost" onClick={() => setToasts([])}>Clear all</Btn>
        </div>

        <Divider />

        {/* Static specimen row showing each tone (always visible reference) */}
        <div className="grid grid-cols-2 gap-3">
          <Toast tone="success" title="Service deployed" description="price-oracle-mainnet is live." />
          <Toast tone="info"    title="New version available" description="WAVS v0.5.1 — restart to apply." action={<Btn size="sm" variant="primary">Restart</Btn>} onDismiss={() => undefined} />
          <Toast tone="warning" title="Aggregator pausing" description="Resuming in 30s." />
          <Toast tone="danger"  title="Submission failed" description="Insufficient quorum." action={<Btn size="sm" variant="danger">Retry</Btn>} onDismiss={() => undefined} />
        </div>

        <p className="text-xs text-ink-fg-muted">
          Successful toasts auto-dismiss in 5s. Errors are sticky — they require user acknowledgment so the operator never misses a failure.
        </p>
      </Surface>

      {/* Live stack — only renders when toasts exist */}
      {toasts.length > 0 && (
        <ToastStack position="br">
          {toasts.map((t) => (
            <Toast
              key={t.id}
              tone={t.tone}
              title={t.title}
              description={t.description}
              action={t.action ? <Btn size="sm" variant={t.tone === 'danger' ? 'danger' : 'primary'} onClick={t.action.onClick}>{t.action.label}</Btn> : undefined}
              onDismiss={() => dismiss(t.id)}
            />
          ))}
        </ToastStack>
      )}
    </div>
  );
}

/* ─── Feedback — Form errors ───────────────────────────────────── */

function FormErrorsPattern() {
  const [submitted, setSubmitted] = useState(false);
  return (
    <Surface variant="outline" className="p-8 flex flex-col gap-6">
      {submitted && (
        <Alert
          tone="danger"
          title="3 errors must be resolved before deploying"
          description={
            <ul className="mt-1.5 space-y-0.5 text-xs">
              <li><span className="font-mono text-ink-danger">·</span> <span className="font-mono text-ink-fg">name</span> — required, lowercase</li>
              <li><span className="font-mono text-ink-danger">·</span> <span className="font-mono text-ink-fg">manager</span> — invalid checksum</li>
              <li><span className="font-mono text-ink-danger">·</span> <span className="font-mono text-ink-fg">digest</span> — component not found in registry</li>
            </ul>
          }
          onDismiss={() => setSubmitted(false)}
        />
      )}
      <div className="grid grid-cols-2 gap-x-8 gap-y-6">
        <Field label="Service name" id="fe1" error="Name is required." optional={false}>
          <Input id="fe1" invalid placeholder="my-service" />
        </Field>
        <Field label="Manager address" id="fe2" error="Invalid EIP-55 checksum.">
          <Input id="fe2" invalid mono value="0x742D35cC6634c0532925A3B844bC9E7595F2BD80" />
        </Field>
        <Field label="Component digest" id="fe3" error="Component sha256:e23c5d… not found in local registry.">
          <Input id="fe3" invalid mono value="sha256:e23c5d7f9a1b3c5d7e9f1a3b5c7d9e1f" />
        </Field>
        <Field label="Aggregator threshold" id="fe4" hint="Must be > 50% of operators.">
          <Input id="fe4" placeholder="e.g. 10" />
        </Field>
      </div>
      <Divider />
      <div className="flex items-center justify-between">
        <span className="text-xs text-ink-fg-muted">Three error sites: <span className="font-mono">field caption</span>, <span className="font-mono">form summary</span>, <span className="font-mono">submit toast</span> on rejection.</span>
        <div className="flex items-center gap-2">
          <Btn variant="ghost" onClick={() => setSubmitted(false)}>Reset demo</Btn>
          <Btn variant="primary" onClick={() => setSubmitted(true)}>Deploy</Btn>
        </div>
      </div>
    </Surface>
  );
}

/* ─── Feedback — Error state ───────────────────────────────────── */

function ErrorStatePattern() {
  return (
    <div className="grid grid-cols-2 gap-5">
      {/* Connection / network */}
      <Surface variant="outline" className="p-10 flex flex-col items-center text-center gap-4">
        <div className="h-12 w-12 rounded-ds-pill border border-ink-danger-edge bg-ink-danger-tint flex items-center justify-center text-ink-danger">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <path d="M3 6c4-3 10-3 14 0M5 9c3-2 7-2 10 0M7 12c2-1 4-1 6 0" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
            <path d="M2 2L18 18" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
          </svg>
        </div>
        <div className="flex flex-col gap-1.5">
          <h3 className="text-md text-ink-fg">Lost connection to WAVS node</h3>
          <p className="text-sm text-ink-fg-muted max-w-sm">
            The local node at <Code>127.0.0.1:8000</Code> stopped responding 14 seconds ago. Live data is paused.
          </p>
        </div>
        <div className="flex items-center gap-2 mt-2">
          <Btn variant="primary" leading={<RefreshIcon />}>Reconnect</Btn>
          <Btn variant="ghost">Open logs</Btn>
        </div>
        <span className="font-mono text-xs text-ink-fg-faint mt-2">retrying in 8s…</span>
      </Surface>

      {/* Fetch / load failure */}
      <Surface variant="outline" className="p-10 flex flex-col items-center text-center gap-4">
        <div className="h-12 w-12 rounded-ds-pill border border-ink-warning-edge bg-ink-warning-tint flex items-center justify-center text-ink-warning">
          <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
            <path d="M10 3L18 17H2L10 3Z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
            <path d="M10 8v4M10 14.4v.4" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
          </svg>
        </div>
        <div className="flex flex-col gap-1.5">
          <h3 className="text-md text-ink-fg">Couldn't load operators</h3>
          <p className="text-sm text-ink-fg-muted max-w-sm">
            Stake registry call reverted: <Code>execution reverted: PausedRegistry()</Code>. The contract is paused for an upgrade.
          </p>
        </div>
        <div className="flex items-center gap-2 mt-2">
          <Btn variant="secondary" leading={<RefreshIcon />}>Retry</Btn>
          <Btn variant="ghost">Copy error</Btn>
        </div>
        <span className="font-mono text-xs text-ink-fg-faint mt-2">last attempt 14:22:08 · 2 retries</span>
      </Surface>
    </div>
  );
}

/* ─── Feedback — Confirmation ──────────────────────────────────── */

function ConfirmPattern() {
  const [confirmText, setConfirmText] = useState('');
  const required = 'price-oracle-mainnet';
  const matches = confirmText === required;

  return (
    <div className="grid grid-cols-2 gap-5">
      {/* Soft confirm — reversible */}
      <Surface variant="flat" className="p-6 flex flex-col gap-4">
        <div className="flex items-start gap-3">
          <span className="h-8 w-8 rounded-ds-pill border border-ink-warning-edge bg-ink-warning-tint flex items-center justify-center text-ink-warning shrink-0">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <rect x="3" y="2.5" width="2" height="7" rx="0.5" fill="currentColor" />
              <rect x="9" y="2.5" width="2" height="7" rx="0.5" fill="currentColor" />
            </svg>
          </span>
          <div className="flex flex-col gap-1.5 min-w-0">
            <h3 className="text-md text-ink-fg">Pause this service?</h3>
            <p className="text-sm text-ink-fg-secondary">
              New triggers will queue but not execute. You can resume at any time. <span className="text-ink-fg-muted">Reversible.</span>
            </p>
          </div>
        </div>
        <Divider />
        <div className="flex items-center justify-end gap-2">
          <Btn variant="ghost">Cancel</Btn>
          <Btn variant="secondary" leading={<PauseIcon />}>Pause service</Btn>
        </div>
      </Surface>

      {/* Hard confirm — destructive, type-to-confirm */}
      <Surface variant="flat" className="p-6 flex flex-col gap-4 border-ink-danger-edge">
        <div className="flex items-start gap-3">
          <span className="h-8 w-8 rounded-ds-pill border border-ink-danger-edge bg-ink-danger-tint flex items-center justify-center text-ink-danger shrink-0">
            <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
              <path d="M3 4h8M5.5 4V2.5h3V4M4 4v8h6V4" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" fill="none" />
            </svg>
          </span>
          <div className="flex flex-col gap-1.5 min-w-0">
            <h3 className="text-md text-ink-fg">Delete service?</h3>
            <p className="text-sm text-ink-fg-secondary">
              Removes the local registration and stops monitoring. The on-chain manager contract is <strong className="text-ink-fg">not</strong> affected. <span className="text-ink-danger">Irreversible from this app.</span>
            </p>
          </div>
        </div>
        <Field label={<>Type <Code>{required}</Code> to confirm</>} id="confirm-input">
          <Input id="confirm-input" mono value={confirmText} onChange={setConfirmText} placeholder={required} />
        </Field>
        <div className="flex items-center justify-end gap-2">
          <Btn variant="ghost" onClick={() => setConfirmText('')}>Cancel</Btn>
          <Btn variant="danger" disabled={!matches} leading={<TrashIcon />}>Delete service</Btn>
        </div>
      </Surface>
    </div>
  );
}

/* ─── Navigation — App bar ─────────────────────────────────────── */

function WavsMark() {
  return (
    <span className="flex items-center gap-2">
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
        <circle cx="7" cy="7" r="3" fill="var(--color-accent)" />
        <circle cx="7" cy="7" r="6" stroke="var(--color-accent-edge)" strokeWidth="1" fill="none" />
      </svg>
      <span className="font-mono text-sm uppercase tracking-widest text-ink-fg">WAVS</span>
    </span>
  );
}

const APPBAR_ITEMS_DEFAULT: AppBarItem[] = [
  { key: 'services',   label: 'Services',   icon: <ServicesGlyph />, active: true },
  { key: 'components', label: 'Components', icon: <ComponentsGlyph /> },
  { key: 'activity',   label: 'Activity',   icon: <ActivityGlyph />, badge: <Tag tone="warning" mono>2</Tag> },
  { key: 'logs',       label: 'Logs',       icon: <LogsGlyph /> },
  { key: 'operators',  label: 'Operators',  icon: <OperatorsGlyph /> },
  { key: 'settings',   label: 'Settings',   icon: <SettingsGlyph /> },
];

function AppBarPattern() {
  const [active, setActive] = useState('services');
  const items = APPBAR_ITEMS_DEFAULT.map((it) => ({
    ...it,
    active: it.key === active,
    onClick: () => setActive(it.key),
  }));

  return (
    <div className="flex flex-col gap-6">
      {/* Standard */}
      <Surface variant="outline" className="overflow-hidden">
        <div className="px-4 py-2 border-b border-ink-border bg-ink-surface-sunken flex items-center justify-between">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Standard · responsive</span>
          <span className="font-mono text-xs text-ink-fg-faint">narrow window to see hamburger</span>
        </div>
        <AppBar
          brand={<WavsMark />}
          items={items}
          actions={
            <>
              <Btn size="sm" variant="ghost" leading={<SearchIcon />}>Search · <Kbd>⌘K</Kbd></Btn>
              <Status tone="live" />
            </>
          }
        />
        <div className="p-6 text-center font-mono text-xs text-ink-fg-faint">— page content —</div>
      </Surface>

      {/* Compact */}
      <Surface variant="outline" className="overflow-hidden">
        <div className="px-4 py-2 border-b border-ink-border bg-ink-surface-sunken">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Compact · icon only</span>
        </div>
        <AppBar
          brand={<WavsMark />}
          items={items}
          compact
          actions={<Btn size="sm" variant="primary">Connect wallet</Btn>}
        />
        <div className="p-6 text-center font-mono text-xs text-ink-fg-faint">— page content —</div>
      </Surface>
    </div>
  );
}

/* ─── Navigation — Sidebar ─────────────────────────────────────── */

function SideNavPattern() {
  const [collapsed, setCollapsed] = useState(false);
  const [active, setActive] = useState('services');

  const groups: SideNavGroup[] = [
    {
      label: 'Workspace',
      items: [
        { key: 'services',   label: 'Services',   icon: <ServicesGlyph />,   badge: <Tag tone="success" mono>4</Tag> },
        { key: 'components', label: 'Components', icon: <ComponentsGlyph />, badge: <Tag tone="neutral" mono>6</Tag> },
        { key: 'operators',  label: 'Operators',  icon: <OperatorsGlyph />,  badge: <Tag tone="warning" mono>1</Tag> },
      ],
    },
    {
      label: 'Activity',
      items: [
        { key: 'events', label: 'Events', icon: <ActivityGlyph /> },
        { key: 'logs',   label: 'Logs',   icon: <LogsGlyph /> },
        { key: 'health', label: 'Health', icon: <HeartGlyph /> },
      ],
    },
    {
      label: 'System',
      items: [
        { key: 'settings', label: 'Settings', icon: <SettingsGlyph /> },
        { key: 'help',     label: 'Help',     icon: <HelpGlyph />, disabled: true },
      ],
    },
  ];

  const withClicks = groups.map((g) => ({
    ...g,
    items: g.items.map((it) => ({
      ...it,
      active: it.key === active,
      onClick: () => setActive(it.key),
    })),
  }));

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
      {/* Static / collapsible */}
      <Surface variant="outline" className="overflow-hidden flex flex-col">
        <div className="px-4 py-2 border-b border-ink-border bg-ink-surface-sunken flex items-center justify-between">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Collapsible · grouped</span>
          <Btn size="sm" variant="ghost" onClick={() => setCollapsed((v) => !v)}>
            {collapsed ? 'Expand' : 'Collapse'}
          </Btn>
        </div>
        <div className="flex h-[360px]">
          <SideNav
            brand={collapsed ? <span className="font-mono text-sm text-ink-accent">W</span> : <WavsMark />}
            groups={withClicks}
            collapsed={collapsed}
            onToggleCollapsed={() => setCollapsed((v) => !v)}
            footer={<span className="font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted">v0.5.1</span>}
          />
          <div className="flex-1 p-6 flex items-center justify-center text-center">
            <span className="font-mono text-xs text-ink-fg-faint">— main content —</span>
          </div>
        </div>
      </Surface>

      {/* Inline / TOC sidebar (this design page itself uses one) */}
      <Surface variant="outline" className="overflow-hidden flex flex-col">
        <div className="px-4 py-2 border-b border-ink-border bg-ink-surface-sunken flex items-center justify-between">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Inline · scroll-spy</span>
          <span className="font-mono text-xs text-ink-fg-faint">used by /design TOC</span>
        </div>
        <div className="h-[360px] overflow-hidden flex">
          <div className="w-[160px] border-r border-ink-border py-4 px-3 overflow-y-auto">
            <div className="font-mono text-[10px] uppercase tracking-widest text-ink-fg-muted mb-2">On this page</div>
            <div className="flex flex-col gap-px">
              {['Overview', 'Configuration', 'Triggers', 'Submissions', 'Logs', 'Danger zone'].map((label, i) => (
                <button
                  key={label}
                  className={`text-left text-xs py-1 px-2 rounded-ds-xs transition-colors duration-ds-fast cursor-pointer ${
                    i === 1 ? 'text-ink-accent border-l-2 border-ink-accent -ml-px pl-2' : 'text-ink-fg-muted hover:text-ink-fg'
                  }`}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <div className="flex-1 p-6 flex items-center justify-center">
            <span className="font-mono text-xs text-ink-fg-faint">— scrolled content —</span>
          </div>
        </div>
      </Surface>
    </div>
  );
}

/* ─── Navigation — Breadcrumbs ─────────────────────────────────── */

function BreadcrumbsPattern() {
  return (
    <Surface variant="outline" className="p-6 flex flex-col gap-5">
      <div className="flex flex-col gap-2">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Default</span>
        <Breadcrumbs
          items={[
            { label: 'services',                onClick: () => undefined },
            { label: 'ethereum',                onClick: () => undefined },
            { label: 'price-oracle-mainnet',    current: true },
          ]}
        />
      </div>
      <Divider />
      <div className="flex flex-col gap-2">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Truncated · &gt;4 levels</span>
        <Breadcrumbs
          items={[
            { label: 'workspace',           onClick: () => undefined },
            { label: 'ethereum',            onClick: () => undefined },
            { label: 'mainnet',             onClick: () => undefined },
            { label: 'services',            onClick: () => undefined },
            { label: 'price-oracle',        onClick: () => undefined },
            { label: 'submissions',         onClick: () => undefined },
            { label: '0xab12…cd34',         current: true },
          ]}
        />
      </div>
      <Divider />
      <div className="flex flex-col gap-2">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Custom separator · ›</span>
        <Breadcrumbs
          separator={<span className="font-mono text-ink-fg-faint">›</span>}
          items={[
            { label: 'components',          onClick: () => undefined },
            { label: 'oracle-twap@v0.4.2',  current: true },
          ]}
        />
      </div>
      <Divider />
      <div className="flex flex-col gap-2">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">In context · with title</span>
        <Surface variant="flat" className="p-5 flex flex-col gap-2">
          <Breadcrumbs
            items={[
              { label: 'services',             onClick: () => undefined },
              { label: 'ethereum',             onClick: () => undefined },
              { label: 'price-oracle-mainnet', current: true },
            ]}
          />
          <h3 className="text-xl text-ink-fg">price-oracle-mainnet</h3>
          <span className="text-sm text-ink-fg-secondary">A canonical chain-of-thought TWAP oracle.</span>
        </Surface>
      </div>
    </Surface>
  );
}

/* ─── Navigation — Pagination ──────────────────────────────────── */

function PaginationPattern() {
  const [pageA, setPageA] = useState(1);
  const [pageB, setPageB] = useState(8);
  const [pageC, setPageC] = useState(527);

  return (
    <Surface variant="outline" className="p-6 flex flex-col gap-5">
      <div className="flex flex-col gap-3">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Few pages · 1–5</span>
        <Pagination page={pageA} pageCount={5} onPageChange={setPageA} totalItems={37} pageSize={8} />
      </div>
      <Divider />
      <div className="flex flex-col gap-3">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Mid range · ellipsized</span>
        <Pagination page={pageB} pageCount={42} onPageChange={setPageB} totalItems={336} pageSize={8} />
      </div>
      <Divider />
      <div className="flex flex-col gap-3">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Large · near end</span>
        <Pagination page={pageC} pageCount={527} onPageChange={setPageC} totalItems={4210} pageSize={8} />
      </div>
    </Surface>
  );
}

/* ─── Navigation — Command palette ─────────────────────────────── */

function PalettePattern() {
  const [open, setOpen] = useState(false);

  // Open on ⌘K / Ctrl+K
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOpen(true);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const groups: PaletteGroup[] = [
    {
      label: 'Pages',
      items: [
        { key: 'p-services',   label: 'Services',   icon: <ServicesGlyph />,   description: 'all deployed AVS', keywords: ['avs', 'service'] },
        { key: 'p-components', label: 'Components', icon: <ComponentsGlyph />, description: 'WASM registry' },
        { key: 'p-operators',  label: 'Operators',  icon: <OperatorsGlyph />,  description: 'roster' },
        { key: 'p-events',     label: 'Events',     icon: <ActivityGlyph />,   description: 'live activity' },
        { key: 'p-logs',       label: 'Logs',       icon: <LogsGlyph />,       description: 'diagnostic output' },
        { key: 'p-settings',   label: 'Settings',   icon: <SettingsGlyph /> },
      ],
    },
    {
      label: 'Services',
      items: [
        { key: 's-1', label: 'price-oracle-mainnet',     description: 'live · ethereum',  trailing: <Status tone="live" /> },
        { key: 's-2', label: 'attestation-relay',        description: 'live · ethereum',  trailing: <Status tone="live" /> },
        { key: 's-3', label: 'slashing-monitor',         description: 'live · ethereum',  trailing: <Status tone="live" /> },
        { key: 's-4', label: 'twap-aggregator',          description: 'pending · sepolia', trailing: <Status tone="pending" /> },
        { key: 's-5', label: 'bridge-validator-sepolia', description: 'paused · sepolia',  trailing: <Status tone="paused" /> },
      ],
    },
    {
      label: 'Components',
      items: [
        { key: 'c-1', label: 'oracle-twap',        description: 'sha256:a78bfa6f…', trailing: <Tag tone="neutral" mono>Rust</Tag> },
        { key: 'c-2', label: 'sig-aggregator',     description: 'sha256:d52f3a91…', trailing: <Tag tone="neutral" mono>Rust</Tag> },
        { key: 'c-3', label: 'attestation-verify', description: 'sha256:f8e1b2a3…', trailing: <Tag tone="neutral" mono>Rust</Tag> },
        { key: 'c-4', label: 'risk-engine',        description: 'sha256:c91a7e2d…', trailing: <Tag tone="neutral" mono>AS</Tag> },
      ],
    },
    {
      label: 'Actions',
      items: [
        { key: 'a-1', label: 'Deploy a new service',    icon: <PlusIcon />, keywords: ['create', 'new'] },
        { key: 'a-2', label: 'Upload a component',      icon: <UploadGlyph /> },
        { key: 'a-3', label: 'Pause aggregator',        icon: <PauseIcon /> },
        { key: 'a-4', label: 'Restart node',            icon: <RefreshIcon /> },
      ],
    },
  ];

  return (
    <Surface variant="outline" className="p-8 flex flex-col items-center text-center gap-4">
      <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">⌘K · or click below</div>
      <Btn variant="secondary" leading={<SearchIcon />} onClick={() => setOpen(true)}>
        Open command palette
        <Kbd>⌘</Kbd>
        <Kbd>K</Kbd>
      </Btn>
      <p className="text-sm text-ink-fg-muted max-w-md">
        Fuzzy-search across destinations, services, components, and actions. Try typing <Code>oracle</Code>, <Code>pause</Code>, or <Code>r-engine</Code>.
      </p>
      <CommandPalette open={open} onClose={() => setOpen(false)} groups={groups} />
    </Surface>
  );
}

/* ─── Navigation — Responsive ──────────────────────────────────── */

function ResponsivePattern() {
  return (
    <div className="flex flex-col gap-6">
      {/* Breakpoint contract table */}
      <Surface variant="outline" className="overflow-hidden">
        <table className="w-full">
          <thead>
            <tr className="bg-ink-surface-sunken border-b border-ink-border">
              <th className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Token</th>
              <th className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Min width</th>
              <th className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Pattern</th>
              <th className="px-4 py-2.5 text-left font-mono text-xs uppercase tracking-widest text-ink-fg-muted font-medium">Behavior</th>
            </tr>
          </thead>
          <tbody>
            {[
              { tw: 'sm', px: '640',  pattern: 'small phone',     behavior: 'Single column. Drawer-only nav. No tables — convert to cards.' },
              { tw: 'md', px: '768',  pattern: 'tablet · narrow', behavior: 'Sidebar appears. Headers de-collapse hamburger.' },
              { tw: 'lg', px: '1024', pattern: 'desktop',         behavior: 'Multi-column grids. Side panels available.' },
              { tw: 'xl', px: '1280', pattern: 'wide desktop',    behavior: 'Full canvas. Detail-list-detail layouts.' },
              { tw: '2xl', px: '1536', pattern: 'ultra-wide',     behavior: 'Cap content max-width; let chrome breathe.' },
            ].map((r, i) => (
              <tr key={r.tw} className={i > 0 ? 'border-t border-ink-border' : ''}>
                <td className="px-4 py-2.5"><span className="font-mono text-sm text-ink-fg">{r.tw}</span></td>
                <td className="px-4 py-2.5"><span className="font-mono text-sm text-ink-fg-secondary tabular-nums">{r.px} px</span></td>
                <td className="px-4 py-2.5"><span className="text-sm text-ink-fg-secondary">{r.pattern}</span></td>
                <td className="px-4 py-2.5"><span className="text-xs text-ink-fg-muted">{r.behavior}</span></td>
              </tr>
            ))}
          </tbody>
        </table>
      </Surface>

      {/* Visual viewport demo */}
      <Surface variant="outline" className="p-6 flex flex-col gap-5">
        <div className="flex items-baseline justify-between">
          <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Viewport simulation · current width</span>
          <ViewportProbe />
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          {[
            { tw: 'block md:hidden', label: '< md · drawer', body: 'Hamburger reveals nav drawer over content.' },
            { tw: 'hidden md:block lg:hidden', label: '≥ md · sidebar', body: 'Sidebar appears inline alongside content.' },
            { tw: 'hidden lg:block', label: '≥ lg · multi-column', body: 'Detail panels open as a third column.' },
          ].map((c) => (
            <Surface key={c.label} variant="flat" className={`p-4 ${c.tw}`}>
              <div className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted mb-1">Active</div>
              <div className="text-sm text-ink-fg mb-1">{c.label}</div>
              <div className="text-xs text-ink-fg-muted">{c.body}</div>
            </Surface>
          ))}
        </div>
        <p className="text-xs text-ink-fg-muted">
          Resize the window — the Tauri shell can be dragged to any width and the layout obliges. Section TOC also collapses to a top hamburger below <Code>md</Code>.
        </p>
      </Surface>

      {/* Stack pattern: row → grid → cards */}
      <Surface variant="outline" className="p-6 flex flex-col gap-3">
        <span className="font-mono text-xs uppercase tracking-widest text-ink-fg-muted">Density rule · table → grid → stack</span>
        <p className="text-sm text-ink-fg-secondary">
          Below <Code>md</Code>, dense tables (operators, services, events) re-flow into stacked cards. Numeric columns become metric-row cells inside each card.
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {SERVICES.slice(0, 2).map((s) => (
            <Surface key={s.name} variant="flat" className="p-4 flex flex-col gap-2">
              <div className="flex items-center justify-between">
                <span className="text-sm text-ink-fg">{s.name}</span>
                <Status tone={s.status} />
              </div>
              <Address value={s.manager} truncate={4} />
              <Divider />
              <div className="grid grid-cols-2 gap-2">
                <Stat label="Triggers · 1h" value={s.triggersHr} />
                <Stat label="Operators"     value={s.operators} />
              </div>
            </Surface>
          ))}
        </div>
      </Surface>
    </div>
  );
}

function ViewportProbe() {
  const [w, setW] = useState<number>(typeof window !== 'undefined' ? window.innerWidth : 0);
  useEffect(() => {
    const onResize = () => setW(window.innerWidth);
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);
  const tier = w >= 1536 ? '2xl' : w >= 1280 ? 'xl' : w >= 1024 ? 'lg' : w >= 768 ? 'md' : w >= 640 ? 'sm' : '<sm';
  return (
    <span className="font-mono text-xs text-ink-fg-secondary tabular-nums">
      {w}px · <Tag tone="accent" mono uppercase>{tier}</Tag>
    </span>
  );
}

/* ─── Glyphs (used by app-bar / side-nav) ───────────────────────── */

function ServicesGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor">
      <rect x="1" y="1" width="5" height="5" rx="1" />
      <rect x="8" y="1" width="5" height="5" rx="1" />
      <rect x="1" y="8" width="5" height="5" rx="1" />
      <rect x="8" y="8" width="5" height="5" rx="1" />
    </svg>
  );
}
function ComponentsGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <path d="M7 1L13 4.5v5L7 13L1 9.5v-5L7 1Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
      <path d="M1 4.5L7 8M13 4.5L7 8M7 8v5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function ActivityGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor">
      <polygon points="8,1 2.5,8 6.5,8 5.5,13 11,6 7,6" />
    </svg>
  );
}
function LogsGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="currentColor">
      <rect x="2" y="2.5" width="10" height="1.5" rx="0.7" />
      <rect x="2" y="6.25" width="10" height="1.5" rx="0.7" />
      <rect x="2" y="10" width="6" height="1.5" rx="0.7" />
    </svg>
  );
}
function OperatorsGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <circle cx="7" cy="5" r="2.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M2 12c0.5-2.5 2.5-4 5-4s4.5 1.5 5 4" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function SettingsGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <circle cx="7" cy="7" r="1.8" stroke="currentColor" strokeWidth="1.2" />
      <path d="M7 1.5v1.8M7 10.7v1.8M1.5 7h1.8M10.7 7h1.8M2.8 2.8l1.3 1.3M9.9 9.9l1.3 1.3M2.8 11.2l1.3-1.3M9.9 4.1l1.3-1.3" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function HeartGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <path d="M7 12c-1.2-1-5-3.5-5-6.5C2 3.8 3.3 2.5 5 2.5c1.1 0 1.7.6 2 1.2c0.3-0.6 0.9-1.2 2-1.2c1.7 0 3 1.3 3 3C12 8.5 8.2 11 7 12Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
    </svg>
  );
}
function HelpGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <circle cx="7" cy="7" r="5.5" stroke="currentColor" strokeWidth="1.2" />
      <path d="M5.5 5.5c0-0.8 0.7-1.5 1.5-1.5s1.5 0.7 1.5 1.5c0 1.2-1.5 1.2-1.5 2.5M7 9.8v0.2" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function UploadGlyph() {
  return (
    <svg width="13" height="13" viewBox="0 0 14 14" fill="none">
      <path d="M7 1.5v7m0-7l-2.5 2.5M7 1.5l2.5 2.5M2 12.5h10" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/* ─── Inline icons ──────────────────────────────────────────────── */

function PlusIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M6 1.5v9M1.5 6h9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
    </svg>
  );
}
function ArrowIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M2 6h8m0 0L7 3m3 3L7 9" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function RefreshIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M2 6a4 4 0 0 1 7-2.6M10 6a4 4 0 0 1-7 2.6M9 1.5v2h-2M3 10.5v-2h2" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" fill="none" />
    </svg>
  );
}
function TrashIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M2 3.5h8M4.5 3.5V2.5h3v1M3.5 3.5v6.5h5V3.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" fill="none" />
    </svg>
  );
}
function HashIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M4 1.5l-1 9M9 1.5l-1 9M2 4.5h9M1 7.5h9" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function SearchIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <circle cx="5" cy="5" r="3.2" stroke="currentColor" strokeWidth="1.2" />
      <path d="M7.5 7.5L10 10" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
    </svg>
  );
}
function DotIcon() {
  return (
    <svg width="6" height="6" viewBox="0 0 6 6" fill="currentColor">
      <circle cx="3" cy="3" r="3" />
    </svg>
  );
}
function PauseIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="currentColor">
      <rect x="3" y="2.5" width="2" height="7" rx="0.5" />
      <rect x="7" y="2.5" width="2" height="7" rx="0.5" />
    </svg>
  );
}
function PlayIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="currentColor">
      <path d="M3.5 2.5L9.5 6l-6 3.5z" />
    </svg>
  );
}
function DownloadIcon() {
  return (
    <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
      <path d="M6 1.5v6m0 0L3.5 5.5M6 7.5l2.5-2M2 9.5h8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function TickIcon() {
  return (
    <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
      <path d="M2.5 6.5L5 9L9.5 3.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function ArrowUpIcon() {
  return (
    <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
      <path d="M6 9.5V2.5m0 0L3 5.5M6 2.5l3 3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function ArrowDownIcon() {
  return (
    <svg width="9" height="9" viewBox="0 0 12 12" fill="none">
      <path d="M6 2.5v7m0 0L3 6.5M6 9.5l3-3" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
