<template>
  <div>
    <h3>Check: {{ pid }} | Script: {{ id }}</h3>
    <div v-if="loading" uk-spinner></div>
    {{ content }}
  </div>
</template>

<script>
export default {
  name: "Check",
  props: ["pid", "id"],
  data() {
    return {
      content: null,
      loading: false
    };
  },
  created() {
    this.loading = true;
    fetch(`/api/master/check_script/${this.pid}/${this.id}`, {
      method: "POST",
    })
      .then((data) => data.json())
      .then((data) => {
        this.loading = false;
        if (data.success) {
          this.content = data.content;
        } else {
          console.log(data.error);
          //notify
          this.$root.notification(
            `Error running script: ${data.error}`,
            "danger",
            0
          );
        }
      })
      .catch(() => { this.loading = false; });
  },
};
</script>

<style>
</style>