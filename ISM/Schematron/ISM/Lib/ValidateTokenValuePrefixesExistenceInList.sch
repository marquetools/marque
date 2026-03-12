<?xml version="1.0" encoding="UTF-8"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<!--
    This abstract pattern checks to see if string prefixes embedded in tokens in an attribute of an element exists in a list or 
    matches the pattern defined by the list.

    $context        := the context in which the searchValue exists
    $searchTermList := the set of values which you want to verify is in the list
    $list           := the list in which to search for the searchValue
    $afterText      := a text string to throw out when getting the embedded prefix
    $prefix .       := a string that separates the embedded prefix from the rest of the token value following the prefix. 
    $errMsg         := the error message text to display when the assertion fails
    
    Example usage:
    <sch:pattern is-a="ValidateValueExistenceInList" id="ISM_ID_00530" xmlns:sch="http://purl.oclc.org/dsdl/schematron">  
        <sch:param name="context" value="@ism:SARIdentifier"/>
        <sch:param name="searchTermList" value="."/>
        <sch:param name="list" value="$SARSourceAuthorityList"/>
        <sch:param name="afterText" value="'SAR-'"/>
        <sch:param name="prefix" value="':'"/>
        <sch:param name="errMsg" value="'
    		[ISM-ID-00530][Error] The tokens in @ism:SARIdentifier must start with a substring before : that exists
        in the SAR Source Authorities CVE.
        '"/>
    </sch:pattern>

-->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" abstract="true" id="ValidateTokenValuePrefixesExistenceInList">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        This abstract pattern checks to see if an attribute of an element exists
        in a list or matches the pattern defined by the list. The calling rule must pass the
        context, search term list, attribute value to check, and an error message.</sch:p>
    <sch:rule id="ValidateTokenValuePrefixesExistenceInList-R1" context="$context">
        <sch:assert 
            test="every $searchTerm in tokenize(normalize-space(string($searchTermList)), ' ') satisfies 
            substring-after(substring-before($searchTerm,$prefix),$afterText) = $list 
            or (some $Term in $list satisfies (matches(normalize-space(substring-after(substring-before($searchTerm,$prefix),$afterText)), concat('^', $Term ,'$'))))" 
            flag="error" role="error">
            <sch:value-of select="$errMsg"/>
        </sch:assert>
    </sch:rule>
</sch:pattern>