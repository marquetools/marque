<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="BANNER PORTION VALUECHECK"?>
<!-- Notices - Distribution Notice: 
           This document has been approved for Public Release and is available for use without restriction.
       -->
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00468">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00468][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
        contains a token starting with [KLM-R], then attribute @ism:classification must have
        a value of [TS].
        
        Human Readable: A USA document containing KLM-R subcompartment data must be classified
        TOP SECRET.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
        If the document is an ISM_USGOV_RESOURCE, for each element which
        specifies attribute @ism:SCIcontrols that contains a token starting with [KLM-R], 
        this rule ensures that attribute @ism:classification is 
        specified with a value containing the token [TS].
    </sch:p>
    <sch:rule id="ISM-ID-00468-R1" context="*[$ISM_USGOV_RESOURCE and util:containsAnyTokenMatching(@ism:SCIcontrols, ('^KLM-R-[A-Z]{3}$'))]">
        <sch:assert test="util:containsAnyOfTheTokens(@ism:classification, ('TS'))" flag="error" role="error">
            [ISM-ID-00468][Error] If ISM_USGOV_RESOURCE and attribute @ism:SCIcontrols
            contains a token starting with [KLM-R], then attribute @ism:classification must have
            a value of [TS].
            
            Human Readable: A USA document containing KLM-R subcompartment data must be classified
            TOP SECRET.
        </sch:assert>
    </sch:rule>
</sch:pattern>
