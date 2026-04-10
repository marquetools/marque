<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!--
    This abstract pattern verifies that a term matches a value in a list using regular expression matching.
    
    $context        := the context in which the searchValue exists
    $list           := the list in which to search for the searchValue
    $errMsg         := the error message text to display when the assertion fails
    
    Example usage:
    <sch:pattern is-a="ValueExistsInList" id="IRM_ID_00027" xmlns:sch="http://purl.oclc.org/dsdl/schematron">  
        <sch:param name="context" value="@ism:releasableTo"/>
        <sch:param name="searchTermList" value="."/>
        <sch:param name="list" value="$releasableToList"/>
        <sch:param name="errMsg" value="'
    		[ISM-ID-00265][Error] Any @ism:releasableTo must
    		be a value in CVEnumISMRelTo.xml.
        '"/>
    </sch:pattern> 
-->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValueExistsInList">
   <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      This abstract pattern verifies that a value at a given context matches fome value in a list
      using regular expression matching. The calling rule must pass the context, search term list, attribute value to
      check, and an error message.</sch:p>
   <sch:rule id="ValueExistsInList-R1" context="$context">
      <sch:assert test="some $Term in $list satisfies (matches(normalize-space(.), concat('^',$Term,'$')))"
         flag="error" role="error">
         <sch:value-of select="$errMsg"/>
      </sch:assert>
   </sch:rule>
</sch:pattern>
