
Salsa has two main problems

1. paralelization within a query. As opposed to "picante" - salsa does not
   parallelize inside a given query. This means that we only get paralelization
   at the top level. For example, a markdown page could request 3 images. Those
   are going to happen sequentially because queries do not allow you to run
   fork() inside.
2. Serialization to disk is only partially implemented behind a feature flag it
   seems.


Furthermore, adding a graph traversal step at the beginning instead of using
salsa, kills the point of using pulldown-cmark because now we need to split
parsing and rendering into two steps, which pulldown-cmark does not like.

There is one more problem. What about user defined assets or parameters, such as
a list of posts for a post list. How does salsa know in a nice, user friendly
way that one of the posts has changed, and thus the list needs to be re
computed?

Maybe the best is to go simple. Lets make a basic cache with an sql database, 
then if needed, we can add a graph on top. Once I have a graph, paralelizing
should in theory be easier. 

- the database tracks inputs -> outputs, and dependencies.
- the database needs to track the dependencies though, otherwise we cannot
  know what to invalidate. This requires either reflection or macros. And i dont 
  really want to do either.
