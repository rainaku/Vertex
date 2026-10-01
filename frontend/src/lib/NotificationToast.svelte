<script lang="ts">
  import { t } from './i18n';
  import { CheckCircle2, FolderOpen, X } from 'lucide-svelte';
  import { revealInExplorer } from './tauri-bridge';

  export let message: string = '';
  export let filePath: string = '';
  export let visible: boolean = false;
  export let onClose: () => void = () => {};

  async function handleReveal() {
    if (filePath) {
      await revealInExplorer(filePath);
    }
  }
</script>

{#if visible}
  <div class="toast-container" role="status" aria-live="polite">
    <div class="toast-glass">
      <div class="icon-wrap">
        <CheckCircle2 size={20} />
      </div>
      <div class="toast-content">
        <div class="toast-title">{$t("Chuyển đổi hoàn tất")}</div>
        <div class="toast-desc">{message}</div>
      </div>
      <div class="toast-actions">
        <button class="btn-reveal" on:click={handleReveal} title={$t("Mở thư mục chứa file")}>
          <FolderOpen size={16} />
          <span>{$t("Mở thư mục")}</span>
        </button>
        <button class="btn-close" on:click={onClose} aria-label={$t("Đóng thông báo")}>
          <X size={16} />
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .toast-container {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 100;
    animation: toastSlideUp 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes toastSlideUp {
    from {
      opacity: 0;
      transform: translate(-50%, 16px);
    }
    to {
      opacity: 1;
      transform: translate(-50%, 0);
    }
  }

  .toast-glass {
    display: flex;
    align-items: center;
    gap: 12px;
    background: #000000;
    border: none;
    box-shadow: 0 16px 40px rgba(0, 0, 0, 0.9);
    padding: 10px 16px;
    border-radius: 9999px;
    color: #ffffff;
    font-family: var(--font-family);
    min-width: 320px;
    max-width: 480px;
  }

  .icon-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ffffff;
  }

  .toast-content {
    flex: 1;
    min-width: 0;
  }

  .toast-title {
    font-size: 13px;
    font-weight: 700;
    color: #ffffff;
    line-height: 1.2;
    font-family: var(--font-family);
  }

  .toast-desc {
    font-size: 11px;
    color: #a1a1aa;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
    font-family: var(--font-family);
  }

  .toast-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .btn-reveal {
    display: flex;
    align-items: center;
    gap: 6px;
    background: #ffffff;
    border: none;
    color: #000000;
    font-family: var(--font-family);
    font-size: 12px;
    font-weight: 700;
    padding: 5px 12px;
    border-radius: 9999px;
    cursor: pointer;
    transition: background 0.15s, color 0.15s, transform 0.1s;
  }

  .btn-reveal:hover {
    background: #e4e4e7;
    transform: scale(1.03);
  }

  .btn-close {
    background: transparent;
    border: none;
    color: #ffffff;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border-radius: 50%;
    transition: background 0.15s, color 0.15s;
  }

  .btn-close:hover {
    color: #000000;
    background: #ffffff;
  }
</style>
