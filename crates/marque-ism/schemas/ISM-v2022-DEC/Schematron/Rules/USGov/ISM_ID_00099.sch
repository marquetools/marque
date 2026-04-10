<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00099">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00099][Error] If ISM_USGOV_RESOURCE and attribute @ism:ownerProducer
        contains the token [FGI], then the token [FGI] must be the only value in attribute @ism:ownerProducer.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribtue @ism:ownerProducer with a value containing the token
        [FGI] this rule ensures that attribute @ism:ownerProducer only contains a 
        single token.
    </sch:p>
    <sch:rule id="ISM-ID-00099-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyOfTheTokens(@ism:ownerProducer, ('FGI'))]">
        <sch:assert test="count(tokenize(normalize-space(string(@ism:ownerProducer)), ' ')) = 1" flag="error" role="error">
            [ISM-ID-00099][Error] If ISM_USGOV_RESOURCE and attribute @ism:ownerProducer
            contains the token [FGI], then the token [FGI] must be the only value in attribute @ism:ownerProducer.
        </sch:assert>
    </sch:rule>
</sch:pattern>