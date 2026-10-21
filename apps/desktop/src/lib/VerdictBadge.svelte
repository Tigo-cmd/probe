<script>
  import { verdict } from './present.js';
  let { value, size = 'sm' } = $props();
</script>

<span class="badge {value} {size}">{verdict(value).label}</span>

<style>
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-weight: 600;
    border-radius: 999px;
    white-space: nowrap;
    justify-self: start;
    letter-spacing: 0;
  }
  .badge::before {
    content: '';
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }
  .sm {
    font-size: 12px;
    padding: 2px 9px 2px 8px;
  }
  .lg {
    font-size: 14px;
    padding: 4px 12px 4px 10px;
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
  }
  .not_graded::before {
    background: none;
    border: 1.5px solid currentColor;
    width: 5px;
    height: 5px;
  }
</style>
