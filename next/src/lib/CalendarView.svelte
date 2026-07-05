<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import { getCalendar, getLibraryStats, type CalendarEntry, type LibraryStats } from './api';

  const dispatch = createEventDispatcher<{ select: { anime_id: number } }>();

  let entries: CalendarEntry[] = [];
  let stats: LibraryStats | null = null;
  let loading = true;
  let error: string | null = null;

  let viewDate = new Date(); // current month being viewed
  $: year = viewDate.getFullYear();
  $: month = viewDate.getMonth(); // 0-11

  // Month grid | Agenda list, remembered across restarts.
  function loadView(): 'month' | 'agenda' {
    try { return (localStorage.getItem('anivault-calendar-view') as 'month' | 'agenda') || 'month'; }
    catch { return 'month'; }
  }
  let viewMode: 'month' | 'agenda' = loadView();
  $: try { localStorage.setItem('anivault-calendar-view', viewMode); } catch {}

  // Live clock (seconds) driving the countdowns; ticks once a second.
  let now = Math.floor(Date.now() / 1000);
  let ticker: ReturnType<typeof setInterval>;

  async function load() {
    loading = true; error = null;
    try {
      [entries, stats] = await Promise.all([getCalendar(), getLibraryStats()]);
    } catch(e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  function prevMonth() { viewDate = new Date(year, month - 1, 1); }
  function nextMonth() { viewDate = new Date(year, month + 1, 1); }

  // Get entries for a specific day of the viewed month.
  function entriesForDay(day: number): CalendarEntry[] {
    return entries.filter(e => {
      if (!e.airing_at) return false;
      const d = new Date(e.airing_at * 1000);
      return d.getFullYear() === year && d.getMonth() === month && d.getDate() === day;
    });
  }

  // ── Agenda: upcoming releases (today onward) grouped by day ────────────────
  function startOfTodaySec(): number {
    const t = new Date(); t.setHours(0, 0, 0, 0);
    return Math.floor(t.getTime() / 1000);
  }
  $: agendaGroups = (() => {
    const from = startOfTodaySec();
    const upcoming = entries
      .filter(e => e.airing_at != null && (e.airing_at as number) >= from)
      .sort((a, b) => (a.airing_at as number) - (b.airing_at as number));
    const groups: { label: string; items: CalendarEntry[] }[] = [];
    let currentKey = '';
    for (const e of upcoming) {
      const d = new Date((e.airing_at as number) * 1000);
      const key = d.toDateString();
      if (key !== currentKey) {
        currentKey = key;
        groups.push({ label: dayLabel(d), items: [] });
      }
      groups[groups.length - 1].items.push(e);
    }
    return groups;
  })();

  function dayLabel(d: Date): string {
    const today = new Date(); today.setHours(0, 0, 0, 0);
    const target = new Date(d); target.setHours(0, 0, 0, 0);
    const diffDays = Math.round((target.getTime() - today.getTime()) / 86_400_000);
    const base = d.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
    if (diffDays === 0) return `Today · ${base}`;
    if (diffDays === 1) return `Tomorrow · ${base}`;
    return base;
  }

  function timeLabel(airingAt: number): string {
    return new Date(airingAt * 1000).toLocaleTimeString(undefined, { hour: 'numeric', minute: '2-digit' });
  }

  // "2d 5h" / "5h 23m" / "23m 10s" — depends on how far out it is.
  function countdown(airingAt: number): string {
    const diff = airingAt - now;
    if (diff <= 0) return 'Aired';
    const d = Math.floor(diff / 86400);
    const h = Math.floor((diff % 86400) / 3600);
    const m = Math.floor((diff % 3600) / 60);
    const s = diff % 60;
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${m}m`;
    if (m > 0) return `${m}m ${s}s`;
    return `${s}s`;
  }
  function countdownLabel(e: CalendarEntry): string {
    if (e.airing_at == null) return '';
    return e.airing_at <= now ? 'Aired' : `in ${countdown(e.airing_at)}`;
  }
  // Airing within 24h → highlight.
  function isSoon(e: CalendarEntry): boolean {
    return e.airing_at != null && e.airing_at > now && (e.airing_at - now) < 86400;
  }

  // ── Hover tooltip (full title + poster) ────────────────────────────────────
  let tip: { entry: CalendarEntry; x: number; y: number } | null = null;
  function placeTip(entry: CalendarEntry, clientX: number, clientY: number) {
    const x = Math.min(clientX + 14, window.innerWidth - 250);
    const y = Math.min(clientY + 16, window.innerHeight - 140);
    tip = { entry, x: Math.max(8, x), y: Math.max(8, y) };
  }
  function showTipAt(entry: CalendarEntry, el: HTMLElement) {
    const r = el.getBoundingClientRect();
    placeTip(entry, r.left, r.bottom);
  }
  function hideTip() { tip = null; }

  function selectEntry(e: CalendarEntry) {
    if (e.anime_id > 0) dispatch('select', { anime_id: e.anime_id });
  }

  $: firstDay = new Date(year, month, 1).getDay();
  $: daysInMonth = new Date(year, month + 1, 0).getDate();
  $: today = new Date();
  $: isToday = (d: number) => today.getFullYear() === year && today.getMonth() === month && today.getDate() === d;

  const monthNames = ['January','February','March','April','May','June','July','August','September','October','November','December'];
  const dayNames = ['Sun','Mon','Tue','Wed','Thu','Fri','Sat'];

  onMount(() => {
    load();
    ticker = setInterval(() => { now = Math.floor(Date.now() / 1000); }, 1000);
  });
  onDestroy(() => clearInterval(ticker));
</script>

<div class="calendar-view">
  <div class="cal-nav">
    {#if viewMode === 'month'}
      <button on:click={prevMonth} aria-label="Previous month">◀</button>
      <h2>{monthNames[month]} {year}</h2>
      <button on:click={nextMonth} aria-label="Next month">▶</button>
    {:else}
      <h2>Agenda</h2>
    {/if}
    {#if stats}<span class="cal-subtitle">{stats.watching} watching</span>{/if}
    <div class="view-toggle" role="tablist" aria-label="Calendar view">
      <button role="tab" aria-selected={viewMode === 'month'} class:active={viewMode === 'month'} on:click={() => viewMode = 'month'}>Month</button>
      <button role="tab" aria-selected={viewMode === 'agenda'} class:active={viewMode === 'agenda'} on:click={() => viewMode = 'agenda'}>Agenda</button>
    </div>
  </div>

  {#if loading}
    <div class="cal-skeleton">{#each Array(6) as _}<div class="skeleton-card"></div>{/each}</div>
  {:else if error}
    <div class="message error"><p>{error}</p><button class="action-btn" on:click={load}>Retry</button></div>
  {:else if viewMode === 'month'}
    <div class="cal-weekdays">
      {#each dayNames as day}
        <div class="cal-day-header">{day}</div>
      {/each}
    </div>
    <div class="cal-grid">
      {#each Array(firstDay) as _}
        <div class="cal-day-cell empty"></div>
      {/each}
      {#each Array(daysInMonth) as _, i}
        {@const d = i + 1}
        {@const dayEntries = entriesForDay(d)}
        <div class="cal-day-cell" class:today={isToday(d)} class:has-entries={dayEntries.length > 0}>
          <span class="cal-day-num">{d}</span>
          {#each dayEntries as entry}
            <div
              class="cal-day-entry"
              tabindex="0"
              role="button"
              aria-label="{entry.title} Ep {entry.next_episode ?? '?'}"
              on:click={() => selectEntry(entry)}
              on:keydown={(e) => e.key === 'Enter' && selectEntry(entry)}
              on:mouseenter={(e) => placeTip(entry, e.clientX, e.clientY)}
              on:mousemove={(e) => placeTip(entry, e.clientX, e.clientY)}
              on:mouseleave={hideTip}
              on:focus={(e) => showTipAt(entry, e.currentTarget)}
              on:blur={hideTip}
            >
              <span class="cal-entry-title">{entry.title}</span>
              {#if entry.next_episode}
                <span class="cal-entry-ep">Ep{entry.next_episode}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {:else}
    <!-- Agenda -->
    <div class="agenda">
      {#if agendaGroups.length === 0}
        <p class="agenda-empty">No upcoming releases for your list in the next couple of months.</p>
      {:else}
        {#each agendaGroups as group}
          <div class="agenda-group">
            <h3 class="agenda-date">{group.label}</h3>
            {#each group.items as e}
              <button
                class="agenda-row"
                on:click={() => selectEntry(e)}
                on:mouseenter={(ev) => e.image_url && placeTip(e, ev.clientX, ev.clientY)}
                on:mousemove={(ev) => e.image_url && placeTip(e, ev.clientX, ev.clientY)}
                on:mouseleave={hideTip}
              >
                {#if e.image_url}
                  <img class="agenda-poster" src={e.image_url} alt="" loading="lazy" />
                {:else}
                  <div class="agenda-poster placeholder"></div>
                {/if}
                <div class="agenda-info">
                  <span class="agenda-title">{e.title}</span>
                  <span class="agenda-sub">
                    {#if e.next_episode}Ep {e.next_episode} · {/if}{#if e.airing_at}{timeLabel(e.airing_at)}{/if}
                  </span>
                </div>
                <span class="agenda-countdown" class:soon={isSoon(e)} class:aired={e.airing_at != null && e.airing_at <= now}>
                  {countdownLabel(e)}
                </span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

{#if tip}
  <div class="cal-tooltip" style="left:{tip.x}px; top:{tip.y}px">
    {#if tip.entry.image_url}
      <img class="tip-poster" src={tip.entry.image_url} alt="" />
    {/if}
    <div class="tip-body">
      <p class="tip-title">{tip.entry.title}</p>
      <p class="tip-meta">
        {#if tip.entry.next_episode}Episode {tip.entry.next_episode}{/if}
        {#if tip.entry.episode_count} / {tip.entry.episode_count}{/if}
      </p>
      {#if tip.entry.airing_at}
        <p class="tip-when">
          {new Date(tip.entry.airing_at * 1000).toLocaleString(undefined, { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' })}
        </p>
        {#if tip.entry.airing_at > now}
          <p class="tip-countdown">in {countdown(tip.entry.airing_at)}</p>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .calendar-view { display: flex; flex-direction: column; gap: 0.6rem; height: 100%; min-height: 34rem; }
  .cal-nav { display: flex; align-items: center; gap: 0.75rem; flex-shrink: 0; }
  .cal-weekdays { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); gap: 1px; flex-shrink: 0; }
  .cal-nav h2 { font-size: 1.3rem; font-weight: 700; min-width: 9rem; }
  .cal-nav button { border: 1px solid rgba(143,183,255,0.2); border-radius: 999px; padding: 0.35rem 0.65rem; background: transparent; color: var(--color-muted); cursor: pointer; }
  .cal-nav button:hover { background: rgba(143,183,255,0.1); color: var(--color-text); }
  .cal-subtitle { color: var(--color-muted); font-size: 0.85rem; }

  .view-toggle { margin-left: auto; display: inline-flex; border: 1px solid rgba(143,183,255,0.2); border-radius: 999px; overflow: hidden; }
  .view-toggle button { border: none; border-radius: 0; padding: 0.35rem 0.9rem; background: transparent; color: var(--color-muted); font-size: 0.8rem; }
  .view-toggle button.active { background: var(--color-accent); color: #06121f; font-weight: 600; }

  .cal-grid { display: grid; grid-template-columns: repeat(7, minmax(0, 1fr)); grid-auto-rows: minmax(4.5rem, 1fr); gap: 1px; background: rgba(143,183,255,0.1); border-radius: 8px; overflow: hidden; flex: 1 1 auto; min-height: 0; }
  .cal-day-header { padding: 0.4rem; text-align: center; font-size: 0.72rem; color: var(--color-muted); font-weight: 600; text-transform: uppercase; background: rgba(143,183,255,0.06); }
  .cal-day-cell { min-width: 0; overflow-y: auto; min-height: 0; padding: 0.25rem 0.3rem; background: rgba(10,13,20,0.9); font-size: 0.72rem; }
  .cal-day-cell.empty { background: rgba(10,13,20,0.5); }
  .cal-day-cell.today { background: rgba(143,183,255,0.08); }
  .cal-day-cell.today .cal-day-num { color: var(--color-accent); font-weight: 700; }
  .cal-day-num { display: block; margin-bottom: 0.15rem; color: var(--color-muted); }
  .cal-day-entry { display: flex; justify-content: space-between; font-size: 0.65rem; padding: 0.1rem 0; overflow: hidden; cursor: pointer; }
  .cal-day-entry:hover { background: rgba(143,183,255,0.1); border-radius: 2px; }
  .cal-entry-title { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; flex: 1; min-width: 0; }
  .cal-entry-ep { color: var(--color-accent); flex-shrink: 0; margin-left: 0.2rem; }

  /* Agenda */
  .agenda { flex: 1 1 auto; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 0.75rem; padding-right: 0.25rem; }
  .agenda-empty { color: var(--color-muted); padding: 1.5rem 0.5rem; text-align: center; }
  .agenda-group { display: flex; flex-direction: column; gap: 0.35rem; }
  .agenda-date { position: sticky; top: 0; z-index: 1; font-size: 0.8rem; font-weight: 700; color: var(--color-text); padding: 0.3rem 0.1rem; background: var(--color-bg, #0a0d14); text-transform: uppercase; letter-spacing: 0.03em; }
  .agenda-row { display: flex; align-items: center; gap: 0.7rem; width: 100%; text-align: left; padding: 0.4rem 0.5rem; border: 1px solid rgba(143,183,255,0.1); border-radius: 10px; background: rgba(143,183,255,0.03); color: var(--color-text); cursor: pointer; }
  .agenda-row:hover { background: rgba(143,183,255,0.1); }
  .agenda-poster { width: 34px; height: 48px; object-fit: cover; border-radius: 5px; flex-shrink: 0; background: rgba(255,255,255,0.05); }
  .agenda-poster.placeholder { display: block; }
  .agenda-info { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .agenda-title { font-size: 0.9rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .agenda-sub { font-size: 0.78rem; color: var(--color-muted); }
  .agenda-countdown { flex-shrink: 0; font-size: 0.78rem; font-variant-numeric: tabular-nums; color: var(--color-muted); padding: 0.2rem 0.55rem; border-radius: 999px; border: 1px solid rgba(143,183,255,0.15); }
  .agenda-countdown.soon { color: #06121f; background: var(--color-accent); border-color: transparent; font-weight: 700; }
  .agenda-countdown.aired { opacity: 0.55; }

  /* Hover tooltip */
  .cal-tooltip { position: fixed; z-index: 50; display: flex; gap: 0.6rem; max-width: 320px; padding: 0.6rem; border-radius: 10px; background: rgba(14,18,28,0.98); border: 1px solid rgba(143,183,255,0.25); box-shadow: 0 8px 24px rgba(0,0,0,0.5); pointer-events: none; }
  .tip-poster { width: 56px; height: 80px; object-fit: cover; border-radius: 6px; flex-shrink: 0; }
  .tip-body { display: flex; flex-direction: column; gap: 0.15rem; min-width: 0; }
  .tip-title { font-size: 0.9rem; font-weight: 700; color: var(--color-text); }
  .tip-meta { font-size: 0.78rem; color: var(--color-muted); }
  .tip-when { font-size: 0.78rem; color: var(--color-text); }
  .tip-countdown { font-size: 0.78rem; color: var(--color-accent); font-weight: 600; font-variant-numeric: tabular-nums; }

  .cal-skeleton { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.5rem; }
  .skeleton-card { height: 3rem; border-radius: 8px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
  .message.error { color: #ff9d9d; padding: 1rem; border: 1px solid rgba(255,157,157,0.2); border-radius: 10px; background: rgba(255,157,157,0.06); }
  .action-btn { border: 1px solid rgba(143,183,255,0.3); border-radius: 999px; padding: 0.35rem 0.75rem; background: rgba(143,183,255,0.1); color: var(--color-text); cursor: pointer; margin-top: 0.5rem; }

  @media (max-width: 900px) {
    .cal-grid { font-size: 0.65rem; }
    .cal-day-cell { min-height: 2.5rem; padding: 0.15rem; }
    .cal-day-num { font-size: 0.65rem; }
    .cal-entry-title { display: none; }
    .cal-day-entry { justify-content: center; }
    .cal-day-header { font-size: 0.6rem; padding: 0.2rem; }
  }

  @media (max-width: 600px) {
    .cal-grid { font-size: 0.55rem; }
    .cal-day-cell { min-height: 1.8rem; padding: 0.1rem; }
    .cal-day-num { font-size: 0.55rem; }
    .cal-entry-ep { font-size: 0.5rem; }
    .cal-nav h2 { font-size: 1rem; min-width: 6rem; }
  }
</style>
