import { afterEach, describe, expect, it } from '@rstest/core';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { Route, Routes } from 'react-router-dom';
import { App } from './App';
import { site } from './config';
import { docNav, getDocPage } from './docs/content';
import { getTmuxGuide } from './docs/tmux-guide';
import { DocPage } from './pages/DocPage';
import { DocsIndex } from './pages/DocsIndex';
import { DocsLayout } from './pages/DocsLayout';
import { HubPage } from './pages/HubPage';
import { renderWithProviders } from './test/render';

afterEach(() => {
  window.history.replaceState({}, '', '/');
});

describe('App hub page', () => {

  it('opens and closes the mobile product navigation', () => {
    renderWithProviders(<App />, { router: false });

    const trigger = screen.getByRole('button', { name: 'Open navigation' });
    fireEvent.click(trigger);
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('navigation', { name: 'Product mobile' })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('navigation', { name: 'Product mobile' })).not.toBeInTheDocument();
  });

  it('keeps Chinese selected while navigating every app surface', async () => {
    renderWithProviders(<App />, { router: false });

    const expectChineseLocale = () => {
      expect(document.documentElement.lang).toBe('zh-Hans');
      expect(screen.getByRole('menuitemradio', { name: '中文' })).toHaveAttribute(
        'aria-checked',
        'true',
      );
      expect(screen.getByLabelText('切换语言')).toBeInTheDocument();
      expect(screen.getByRole('navigation', { name: '产品' })).toBeInTheDocument();
      expect(screen.getByRole('navigation', { name: '页脚' })).toBeInTheDocument();
      expect(screen.queryByLabelText('Switch language')).not.toBeInTheDocument();
      expect(screen.queryByRole('navigation', { name: 'Product' })).not.toBeInTheDocument();
      expect(screen.queryByRole('navigation', { name: 'Footer' })).not.toBeInTheDocument();
    };
    const expectChineseHeading = (englishHeading: RegExp) => {
      expectChineseLocale();
      expect(
        screen.getByRole('heading', { level: 1, name: /[\u3400-\u9fff]/u }),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole('heading', { level: 1, name: englishHeading }),
      ).not.toBeInTheDocument();
    };
    const productNav = () => screen.getByRole('navigation', { name: '产品' });

    fireEvent.click(screen.getByRole('menuitemradio', { name: '中文' }));
    expectChineseLocale();
    expect(screen.getByRole('heading', { level: 1, name: 'Nerve' })).toBeInTheDocument();
    expect(screen.getByText(/开放神经系统/)).toBeInTheDocument();
    expect(screen.getByText(/跨机器长时任务监控器/)).toBeInTheDocument();
    expect(screen.getAllByText('出问题').length).toBeGreaterThan(0);
    expect(screen.getAllByText('需要你').length).toBeGreaterThan(0);
    expect(screen.queryByText('Problem')).not.toBeInTheDocument();
    expect(screen.queryByText('Attention')).not.toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /一个任务列表.*覆盖每台机器/ })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /同一批任务.*VS Code/ })).toBeInTheDocument();

    fireEvent.click(within(productNav()).getByRole('link', { name: '文档' }));
    await waitFor(() => expect(window.location.pathname).toBe('/docs'));
    expectChineseHeading(/The whole protocol, written down/i);

    const getStartedLink = document.querySelector<HTMLAnchorElement>(
      'li > a[href="/docs/get-started"]',
    );
    expect(getStartedLink).not.toBeNull();
    fireEvent.click(getStartedLink as HTMLAnchorElement);
    await waitFor(() => expect(window.location.pathname).toBe('/docs/get-started'));
    expectChineseHeading(/^Get started$/i);
  });

  it('redirects retired surface routes onto homepage sections', async () => {
    window.history.replaceState({}, '', '/macos');
    const macos = renderWithProviders(<App />, { router: false });
    await waitFor(() => {
      expect(window.location.pathname).toBe('/');
      expect(window.location.hash).toBe('#macos');
    });
    macos.unmount();

    window.history.replaceState({}, '', '/tmux');
    const tmux = renderWithProviders(<App />, { router: false });
    await waitFor(() => {
      expect(window.location.pathname).toBe('/');
      expect(window.location.hash).toBe('#tmux');
    });
    tmux.unmount();

    window.localStorage.setItem('nerve-locale', 'zh');
    window.history.replaceState({}, '', '/tmux/zh');
    renderWithProviders(<App />, { router: false });
    await waitFor(() => {
      expect(window.location.pathname).toBe('/');
      expect(window.location.hash).toBe('#tmux');
    });
  });
});

describe('Docs subpages', () => {
  it('lists all handbook cards on the docs index', () => {
    renderWithProviders(
      <Routes>
        <Route path="/docs" element={<DocsLayout />}>
          <Route index element={<DocsIndex />} />
        </Route>
      </Routes>,
      { router: { initialEntries: ['/docs'] } },
    );

    expect(screen.getByRole('heading', { name: /The whole protocol/i })).toBeInTheDocument();
    for (const item of docNav) {
      expect(screen.getByRole('link', { name: new RegExp(`${item.title}.*${item.summary}`, 'i') })).toBeInTheDocument();
    }
  });

});
