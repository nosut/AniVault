<script lang="ts">
  import { onMount } from 'svelte';
  import { getStatistics, type AnimeStats } from './api';

  let stats: AnimeStats | null = null;
  let loading = true;
  let error: string | null = null;

  async function load() {
    loading = true; error = null;
    try { stats = await getStatistics(); }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { loading = false; }
  }

  function maxScoreCount(): number {
    if (!stats) return 1;
    return Math.max(1, ...stats.score_distribution.map((b: { count: number }) => b.count));
  }

  onMount(load);
</script>

<div class="stats-view">
  <h2>Statistics</h2>

  {#if loading}
    <div class="skeleton-grid">
      {#each Array(4) as _}<div class="skeleton-card" />{/each}
    </div>
  {:else if error}
    <div class="message error" role="alert"><p>{error}</p><button class="action-btn" on:click={load}>Retry</button></div>
  {:else if stats}
    <div class="summary-grid">
      <div class="stat-card"><div class="stat-value">{stats.total_anime}</div><div class="stat-label">Total Anime</div></div>
      <div class="stat-card"><div class="stat-value">{stats.total_episodes_watched}</div><div class="stat-label">Episodes Watched</div></div>
      <div class="stat-card"><div class="stat-value">{stats.total_rewatches}</div><div class="stat-label">Watch Events</div></div>
      <div class="stat-card"><div class="stat-value">{stats.avg_score.toFixed(1)}</div><div class="stat-label">Avg Score</div></div>
    </div>

    <div class="activity-grid">
      <div class="stat-card"><div class="stat-value">{stats.episodes_today}</div><div class="stat-label">Episodes Today</div></div>
      <div class="stat-card"><div class="stat-value">{stats.episodes_this_week}</div><div class="stat-label">Episodes This Week</div></div>
    </div>

    <section class="score-section">
      <h3>Score Distribution</h3>
      <div class="score-chart">
        {#each stats.score_distribution as bucket}
          <div class="score-row">
            <span class="score-label">{bucket.range}</span>
            <div class="score-bar-wrap">
              <div class="score-bar" style="width: {bucket.count / maxScoreCount() * 100}%" />
            </div>
            <span class="score-count">{bucket.count}</span>
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .stats-view { display: flex; flex-direction: column; gap: 1.5rem; }
  h2 { font-size: 1.3rem; font-weight: 700; }
  h3 { font-size: 1rem; font-weight: 600; margin-bottom: 0.75rem; }
  .summary-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: 0.75rem; }
  .activity-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr)); gap: 0.75rem; }
  .stat-card { border: 1px solid rgba(143,183,255,0.12); border-radius: 10px; padding: 1rem; background: rgba(255,255,255,0.03); text-align: center; }
  .stat-value { font-size: 1.5rem; font-weight: 700; color: var(--color-accent); }
  .stat-label { font-size: 0.78rem; color: var(--color-muted); margin-top: 0.25rem; }
  .score-section { border: 1px solid rgba(143,183,255,0.1); border-radius: 10px; padding: 1rem; background: rgba(255,255,255,0.02); }
  .score-chart { display: flex; flex-direction: column; gap: 0.5rem; }
  .score-row { display: flex; align-items: center; gap: 0.75rem; }
  .score-label { width: 4rem; font-size: 0.82rem; color: var(--color-muted); text-align: right; }
  .score-bar-wrap { flex: 1; height: 1rem; border-radius: 4px; background: rgba(255,255,255,0.06); overflow: hidden; }
  .score-bar { height: 100%; border-radius: 4px; background: linear-gradient(90deg, rgba(143,183,255,0.4), rgba(143,183,255,0.7)); transition: width 0.5s ease; min-width: 2px; }
  .score-count { width: 2.5rem; font-size: 0.82rem; font-weight: 600; text-align: left; }
  .skeleton-grid { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.75rem; }
  .skeleton-card { height: 5rem; border-radius: 10px; background: rgba(255,255,255,0.04); animation: pulse 2s infinite; }
  @keyframes pulse { 0%,100%{opacity:0.4} 50%{opacity:0.7} }
  .message.error { color: #ff9d9d; padding: 1rem; border: 1px solid rgba(255,157,157,0.2); border-radius: 10px; background: rgba(255,157,157,0.06); }
  .action-btn { border: 1px solid rgba(143,183,255,0.3); border-radius: 999px; padding: 0.4rem 0.9rem; background: rgba(143,183,255,0.1); color: var(--color-text); cursor: pointer; margin-top: 0.5rem; }
</style>
