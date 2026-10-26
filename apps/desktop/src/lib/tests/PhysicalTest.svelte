<script>
  // What software cannot see: a guided look at the case, hinges, ports and
  // battery. Each item is Fine, Problem (with an optional note) or Skip.
  import { PHYSICAL_ITEMS, physicalResult } from '../handson.js';

  let { oncomplete, oncancel } = $props();
  let answers = $state({});
  let notes = $state({});
  const answered = $derived(PHYSICAL_ITEMS.filter((i) => answers[i.id]).length);
</script>

<div class="test">
  <p class="eyebrow">Physical inspection · {answered} of {PHYSICAL_ITEMS.length}</p>
  <h1>Look the computer over</h1>
  <p class="muted">Software cannot see any of this. Take a minute with the computer in front of you.</p>

  <ul>
    {#each PHYSICAL_ITEMS as item (item.id)}
      <li class="card" class:problem={answers[item.id] === 'problem'}>
        <div class="text">
          <h2>{item.title}</h2>
          <p class="faint small">{item.how}</p>
          {#if answers[item.id] === 'problem'}
            <input type="text" placeholder="What did you notice? (optional)" bind:value={notes[item.id]} maxlength="160" />
          {/if}
        </div>
        <div class="seg" role="radiogroup" aria-label={item.title}>
          {#each [['fine', 'Fine'], ['problem', 'Problem'], ['skip', 'Skip']] as [v, label] (v)}
            <button role="radio" aria-checked={answers[item.id] === v} class="{v} {answers[item.id] === v ? 'on' : ''}" onclick={() => (answers[item.id] = v)}>{label}</button>
          {/each}
        </div>
      </li>
    {/each}
  </ul>

  <div class="buttons">
    <button class="primary" onclick={() => oncomplete(physicalResult(answers, notes))}>Save inspection</button>
    <button class="plain" onclick={oncancel}>Cancel</button>
  </div>
</div>

<style>
  .test {
    max-width: 820px;
    margin: 0 auto;
    padding: 32px 24px 48px;
    display: grid;
    gap: 14px;
  }
  h1 {
    font-size: 30px;
    font-weight: 700;
    letter-spacing: -0.025em;
  }
  ul {
    list-style: none;
    margin: 6px 0 0;
    padding: 0;
    display: grid;
    gap: 10px;
  }
  li {
    padding: 16px 18px;
    display: flex;
    gap: 16px;
    align-items: center;
    justify-content: space-between;
    transition: box-shadow 200ms;
  }
  li.problem {
    box-shadow:
      inset 3px 0 0 var(--red-fill),
      var(--shadow);
  }
  .text {
    display: grid;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }
  h2 {
    font-size: 16px;
    font-weight: 650;
  }
  input {
    font: inherit;
    font-size: 14px;
    margin-top: 4px;
    padding: 7px 10px;
    border-radius: 10px;
    border: 1px solid var(--separator);
    background: var(--surface);
    color: var(--text);
  }
  .seg {
    flex: none;
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: 999px;
    background: var(--surface-2);
  }
  .seg button {
    padding: 5px 12px;
    font-size: 13px;
    background: transparent;
    color: var(--text-3);
  }
  .seg button.on {
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }
  .seg button.fine.on {
    color: var(--green);
  }
  .seg button.problem.on {
    color: var(--red);
  }
  .buttons {
    display: flex;
    gap: 10px;
    align-items: center;
    margin-top: 6px;
  }
  @media (max-width: 620px) {
    li {
      flex-direction: column;
      align-items: stretch;
    }
  }
</style>
