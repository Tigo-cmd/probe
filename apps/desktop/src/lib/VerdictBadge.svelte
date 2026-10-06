<script>
  import { verdict } from './present.js';
  let { value, size = 'sm' } = $props();
</script>

<span class="badge {value} {size}">{verdict(value).label}</span>

<style>
  .badge {
    display: inline-block;
    font-weight: 650;
    letter-spacing: 0.02em;
    border-radius: 999px;
    white-space: nowrap;
    border: 1px solid transparent;
    justify-self: start;
  }
  .sm {
    font-size: 12px;
    padding: 1px 9px;
    min-width: 84px;
    text-align: center;
  }
  .lg {
    font-size: 14px;
    padding: 3px 12px;
  }
  .green {
    color: var(--green);
    background: var(--green-bg);
  }
  .amber {
    color: var(--amber);
    background: var(--amber-bg);
  }
  .red {
    color: var(--red);
    background: var(--red-bg);
  }
  .not_graded {
    color: var(--grey);
    background: var(--grey-bg);
    border-color: var(--border);
  }
</style>
