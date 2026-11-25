import { expect, test } from '@playwright/test';

/* Мок (web/mock/mock-api.mjs): три интерактивных разговора про nginx
 * (claude — открыт в терминале, codex, grok) и один автоматический claude. */

test('поиск → карточки → контекст разговора', async ({ page }) => {
  await page.goto('/?q=n');
  // Порог длины приходит из статуса сервера (minQueryLength).
  await expect(page.getByText('Введите хотя бы 2 символа')).toBeVisible();
  await page.goto('/?q=nginx');
  await expect(page.getByText('Разговоров: 3')).toBeVisible();
  // Команду продолжения собирает бэкенд — и для Windows-пути codex.
  await expect(page.getByText(/codex resume 019a2b3c/)).toBeVisible();
  await expect(page.getByText('Почему nginx отдаёт 502 после деплоя')).toBeVisible();
  await expect(page.getByText('открыт в терминале (pid 48213)')).toBeVisible();

  await page.getByRole('button', { name: 'Контекст' }).first().click();
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByText('Настроить nginx для демо-стенда')).toBeVisible();
  await expect(dialog.locator('mark').first()).toHaveText(/nginx/i);
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();
});

test('фильтр агента и автоматические — в адресе и после перезагрузки', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('searchbox').fill('nginx');
  await expect(page.getByText('Разговоров: 3')).toBeVisible();
  await expect(page).toHaveURL(/\?q=nginx$/);

  const agent = page.locator('.ant-select');
  await agent.click();
  await page.locator('.ant-select-item-option', { hasText: 'Codex' }).click();
  await expect(page.getByText('Разговоров: 1')).toBeVisible();
  await expect(page).toHaveURL(/provider=codex/);

  await agent.click();
  await page.locator('.ant-select-item-option', { hasText: 'Все агенты' }).click();
  await page.getByRole('checkbox', { name: /Включая автоматические/ }).check();
  await expect(page.getByText('Разговоров: 4')).toBeVisible();
  await expect(page.getByText('автоматический')).toBeVisible();
  await expect(page).toHaveURL(/headless=1/);

  await page.reload();
  await expect(page.getByRole('searchbox')).toHaveValue('nginx');
  await expect(page.getByRole('checkbox', { name: /Включая автоматические/ })).toBeChecked();
  await expect(page.getByText('Разговоров: 4')).toBeVisible();
});

test('продолжить, ошибка продолжения, обновить индекс, источники', async ({ page }) => {
  await page.goto('/?q=nginx');
  await expect(page.getByText('Разговоров: 3')).toBeVisible();
  await expect(page.getByText('Уже открыт в терминале')).toBeVisible();

  const grok = page.locator('.ant-card', { hasText: 'Переписать конфиг nginx на шаблоны' });
  await grok.getByRole('button', { name: /Продолжить/ }).click();
  await expect(page.getByText('Открыто в терминале')).toBeVisible();
  await expect(grok.locator('code')).toContainText('grok --resume');
  await expect(grok.getByText('Выполните команду в папке разговора')).toBeVisible();
  await expect(page.getByText('Выполните команду в папке разговора')).toHaveCount(1);

  const codex = page.locator('.ant-card', { hasText: 'Почему nginx отдаёт 502' });
  await codex.getByRole('button', { name: /Продолжить/ }).click();
  await expect(page.getByText(/Папки разговора нет/)).toBeVisible();

  await page.getByRole('button', { name: /Обновить индекс/ }).click();
  await expect(page.getByText('Обновлено разговоров: 2')).toBeVisible();

  await page.getByRole('button', { name: /Источники/ }).click();
  await expect(page.getByText('Какие папки индексируются')).toBeVisible();
  await expect(page.getByText('папки нет')).toBeVisible();
});
