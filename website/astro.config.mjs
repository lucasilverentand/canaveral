// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://lucasilverentand.github.io',
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
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/lucasilverentand/canaveral' },
			],
			editLink: {
				baseUrl: 'https://github.com/lucasilverentand/canaveral/edit/main/website/',
			},
			customCss: [
				'./src/styles/custom.css',
			],
			head: [
				{
					tag: 'meta',
					attrs: {
						property: 'og:image',
						content: 'https://lucasilverentand.github.io/canaveral/og-image.png',
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
						{ label: 'Doctor', slug: 'commands/doctor' },
					],
				},
				{
					label: 'Frameworks',
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
					label: 'Build & Test',
					items: [
						{ label: 'Building', slug: 'commands/build' },
						{ label: 'Testing', slug: 'commands/test' },
					],
				},
				{
					label: 'Versioning',
					items: [
						{ label: 'Version', slug: 'commands/version' },
						{ label: 'Changelog', slug: 'reference/changelog' },
					],
				},
				{
					label: 'Code Signing',
					items: [
						{ label: 'Overview', slug: 'signing/overview' },
						{ label: 'iOS Certificates', slug: 'signing/ios-certificates' },
						{ label: 'Android Keystore', slug: 'signing/android-keystore' },
						{ label: 'Match (Sync)', slug: 'commands/match' },
					],
				},
				{
					label: 'Distribution',
					items: [
						{ label: 'Publish', slug: 'commands/upload' },
						{ label: 'App Store Connect', slug: 'distribution/app-store' },
						{ label: 'Google Play', slug: 'distribution/google-play' },
						{ label: 'TestFlight', slug: 'commands/testflight' },
						{ label: 'Firebase App Distribution', slug: 'distribution/firebase' },
						{ label: 'Metadata', slug: 'commands/metadata' },
						{ label: 'Screenshots', slug: 'commands/screenshots' },
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
