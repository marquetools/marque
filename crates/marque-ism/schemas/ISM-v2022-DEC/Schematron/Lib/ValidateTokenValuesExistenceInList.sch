<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!--
    This abstract pattern checks to see if an attribute of an element exists in a list or matches the pattern defined by the list.

    $context        := the context in which the searchValue exists
    $searchTermList := the set of values which you want to verify is in the list
    $list           := the list in which to search for the searchValue
    $errMsg         := the error message text to display when the assertion fails
    
    Example usage:
    <sch:pattern is-a="ValidateValueExistenceInList" id="IRM_ID_00027" xmlns:sch="http://purl.oclc.org/dsdl/schematron">  
        <sch:param name="context" value="@ism:releasableTo"/>
        <sch:param name="searchTermList" value="."/>
        <sch:param name="list" value="$releasableToList"/>
        <sch:param name="errMsg" value="'
    		[ISM-ID-00265][Error] Any @ism:releasableTo must
    		be a value in CVEnumISMRelTo.xml.
        '"/>
    </sch:pattern>

    Note: $iso4217TrigraphList is defined in the main document, IRM_XML.xml.
-->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValidateTokenValuesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</sch:p>
    <sch:rule id="ValidateTokenValuesExistenceInList-R1" context="$context">
        <sch:assert 
            test="every $searchTerm in tokenize(normalize-space(string($searchTermList)), ' ') satisfies
                   $searchTerm = $list or (some $Term in $list satisfies (matches(normalize-space($searchTerm), concat('^', $Term ,'$'))))" 
            flag="error" role="error">
            <sch:value-of select="$errMsg"/>
        </sch:assert>
    </sch:rule>
</sch:pattern>