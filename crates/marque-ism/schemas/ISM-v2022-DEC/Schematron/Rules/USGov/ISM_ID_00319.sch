<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00319">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00319][Error] If ISM_USGOV_RESOURCE and @ism:ownerProducer contains 'USA' and attribute
        @ism:releasableTo is specified, then @ism:releasableTo must contain more than a single token.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE and a portion's @ism:ownerProducer attribute contains 'USA' and specifies
        attribute @ism:releasableTo, this rule ensures that the token count for releasableTo is greater than
        1.
    </sch:p>
    <sch:rule id="ISM-ID-00319-R1" context="*[util:containsAnyTokenMatching(@ism:ownerProducer, 'USA') and @ism:releasableTo and $ISM_USGOV_RESOURCE]">
        <sch:assert test="count(tokenize(normalize-space(string(@ism:releasableTo)), ' ')) &gt; 1" flag="error" role="error">
            [ISM-ID-00319][Error] If ISM_USGOV_RESOURCE and @ism:ownerProducer contains 'USA' and attribute
            @ism:releasableTo is specified, then @ism:releasableTo must contain more than a single token.
        </sch:assert>
    </sch:rule>
</sch:pattern>