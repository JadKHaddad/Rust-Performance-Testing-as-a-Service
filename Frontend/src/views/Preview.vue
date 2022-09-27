<template>
  <div>
    <div class="check-content text">{{ content }}</div>
  </div>
</template>

<script>
export default {
  name: "Check",
  props: ["pid", "id"],
  data() {
    return {
      content: null,
    };
  },
  created() {
    this.loading = true;
    fetch(`/api/master/preview_script/${this.pid}/${this.id}`, {
      method: "POST",
    })
      .then((data) => data.json())
      .then((data) => {
        if (data.success) {
          this.content = data.content;
        } else {
          
        }
      })
      .catch();
  },
};
</script>

<style>
</style>