// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://anthropics.github.io',
	base: '/canaveral',
	integrations: [
		starlight({
			title: 'Canaveral',
			description: 'Universal Release Management CLI - Build, test, and ship mobile apps with a single tool.',
			components: {
				Hero: './src/components/Hero.astro',
			},
			logo: {
				light: './src/assets/logo-light.svg',
				dark: './src/assets/logo-dark.svg',
				replacesTitle: false,
			},
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/anthropics/canaveral' },
			],
			editLink: {
				baseUrl: 'https://github.com/anthropics/canaveral/edit/main/website/',
			},
			customCss: [
				'./src/styles/custom.css',
			],
			head: [
				{
					tag: 'meta',
					attrs: {
						property: 'og:image',
						content: 'https://anthropics.github.io/canaveral/og-image.png',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'twitter:card',
						content: 'summary_large_image',
					},
				},
			],
			sidebar: [
				{
					label: 'Getting Started',
					items: [
						{ label: 'Introduction', slug: 'getting-started/introduction' },
						{ label: 'Installation', slug: 'getting-started/installation' },
						{ label: 'Quick Start', slug: 'getting-started/quick-start' },
						{ label: 'Configuration', slug: 'getting-started/configuration' },
					],
				},
				{
					label: 'Frameworks',
					collapsed: false,
					items: [
						{ label: 'Overview', slug: 'frameworks/overview' },
						{ label: 'Flutter', slug: 'frameworks/flutter' },
						{ label: 'Expo', slug: 'frameworks/expo' },
						{ label: 'React Native', slug: 'frameworks/react-native' },
						{ label: 'Native iOS', slug: 'frameworks/native-ios' },
						{ label: 'Native Android', slug: 'frameworks/native-android' },
						{ label: 'Tauri', slug: 'frameworks/tauri' },
					],
				},
				{
					label: 'Commands',
					collapsed: true,
					items: [
						{ label: 'build', slug: 'commands/build' },
						{ label: 'test', slug: 'commands/test' },
						{ label: 'version', slug: 'commands/version' },
						{ label: 'upload', slug: 'commands/upload' },
						{ label: 'testflight', slug: 'commands/testflight' },
						{ label: 'match', slug: 'commands/match' },
						{ label: 'screenshots', slug: 'commands/screenshots' },
						{ label: 'metadata', slug: 'commands/metadata' },
						{ label: 'doctor', slug: 'commands/doctor' },
					],
				},
				{
					label: 'Distribution',
					collapsed: true,
					items: [
						{ label: 'App Store Connect', slug: 'distribution/app-store' },
						{ label: 'Google Play', slug: 'distribution/google-play' },
						{ label: 'TestFlight', slug: 'distribution/testflight' },
						{ label: 'Firebase App Distribution', slug: 'distribution/firebase' },
					],
				},
				{
					label: 'CI/CD',
					collapsed: true,
					items: [
						{ label: 'Overview', slug: 'ci-cd/overview' },
						{ label: 'GitHub Actions', slug: 'ci-cd/github-actions' },
						{ label: 'GitLab CI', slug: 'ci-cd/gitlab-ci' },
						{ label: 'Bitrise', slug: 'ci-cd/bitrise' },
						{ label: 'CircleCI', slug: 'ci-cd/circleci' },
					],
				},
				{
					label: 'Code Signing',
					collapsed: true,
					items: [
						{ label: 'Overview', slug: 'signing/overview' },
						{ label: 'iOS Certificates', slug: 'signing/ios-certificates' },
						{ label: 'Android Keystore', slug: 'signing/android-keystore' },
						{ label: 'Match (Sync)', slug: 'signing/match' },
					],
				},
				{
					label: 'Migration',
					collapsed: true,
					items: [
						{ label: 'From Fastlane', slug: 'migration/from-fastlane' },
						{ label: 'From Bitrise Steps', slug: 'migration/from-bitrise' },
					],
				},
				{
					label: 'Reference',
					collapsed: true,
					items: [
						{ label: 'Configuration File', slug: 'reference/configuration' },
						{ label: 'Environment Variables', slug: 'reference/environment-variables' },
						{ label: 'Exit Codes', slug: 'reference/exit-codes' },
						{ label: 'Changelog', slug: 'reference/changelog' },
					],
				},
			],
			expressiveCode: {
				themes: ['dracula', 'github-light'],
				defaultProps: {
					wrap: true,
				},
			},
		}),
	],
});
