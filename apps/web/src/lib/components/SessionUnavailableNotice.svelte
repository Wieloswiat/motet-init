<!--
	The account controls cannot say whether the reader is signed in. Silence
	would leave a signed-in reader looking at a Sign in link, so the notice
	names the missing status instead of guessing at it.
-->
<div class="m-alert__root m-alert__root--tone_caution m-alert__root--variant_inline" role="status">
	<span class="m-icon m-alert__icon m-alert__icon--tone_caution" aria-hidden="true">warning</span>
	<div class="m-alert__content">
		<p class="m-alert__title">Sign-in status unavailable</p>
		<div class="m-alert__description">
			The account service did not answer, so this page shows you as signed out. Reload to try again.
		</div>
	</div>
	<span></span>
</div>
