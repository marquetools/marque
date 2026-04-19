<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!--
    This abstract pattern checks to see if the attribute values of an element exists in a list or matches the pattern defined by the list
    when these attribute values are flagged as contributing to rollup. 

    $context             := the context in which the searchValue exists
    $searchTermList      := the set of values which you want to verify is in the list
    $list                := the list in which to search for the searchValue
    $errMsg              := the error message text to display when the assertion fails
    $contributesToRollup := the boolean that determines whether the attributes values contribute to rollup
--><sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValidateTokenValuesExistenceInListWhenContributesToRollup">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This abstract pattern checks to see if the attribute values of an element 
        exists in a list or matches the pattern defined by the list when these values are flagged as 
        contributing to rollup. The calling rule must pass the context, search term list, attribute value 
        to check, flag on whether the attribute values contribute to rollup, and an error message.</sch:p>
    <sch:rule id="ValidateTokenValuesExistenceInListWhenContributesToRollup-R1" context="$context">
        <sch:assert test="if ($contributesToRollup) then every $searchTerm in tokenize(normalize-space(string($searchTermList)), ' ') satisfies             $searchTerm = $list or (some $Term in $list satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$')))) else true()" flag="error" role="error">
            <sch:value-of select="$errMsg"/>
            The value(s) [<sch:value-of select="string-join(for $searchTerm in tokenize(normalize-space(string($searchTermList)), ' ')                  return if($searchTerm = $list) then null else $searchTerm,' ')"/>] that contribute to rollup could not be found.
        </sch:assert>
    </sch:rule>
</sch:pattern>