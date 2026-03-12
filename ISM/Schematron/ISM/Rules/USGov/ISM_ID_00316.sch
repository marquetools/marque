<?xml version="1.0" encoding="UTF-8"?>
<?ICEA pattern?>
<?schematron-phases phaseids="ROLLDOWN BANNER VALUECHECK"?>
<sch:pattern xmlns:sch="http://purl.oclc.org/dsdl/schematron" id="ISM-ID-00316">
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="ruleText">
        [ISM-ID-00316][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassException of ISM_RESOURCE_ELEMENT contains 
        [NATO] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
        @ism:ownerProducer attribute containing [NATO] or the resource level attribute @ism:FGIsourceOpen must contain [NATO].
        
        Human Readable: USA documents marked with a NATO declass exemption must have NATO portions or FGI NATO at the resource level.
    </sch:p>
    <sch:p xmlns:ism="urn:us:gov:ic:ism" ism:classification="U" ism:ownerProducer="USA" class="codeDesc">
      If the document is an ISM_USGOV_RESOURCE, the current element is the
      ISM_RESOURCE_ELEMENT, and attribute @ism:declassException is specified
      with a value containing the value [NATO], then this rule ensures that some
      element meeting ISM_CONTRIBUTES specifies attribute @ism:ownerProducer
      with a value containing [NATO] or that the resource level @ism:FGIsourceOpen contains [NATO].
    </sch:p>
    <sch:rule id="ISM-ID-00316-R1" context="*[$ISM_USGOV_RESOURCE and generate-id(.) = generate-id($ISM_RESOURCE_ELEMENT) and util:containsAnyOfTheTokens(@ism:declassException, ('NATO'))]">
        <sch:assert test="util:containsAnyTokenMatching(string-join($partOwnerProducer_tok,' '), ('^NATO:?'))
            or util:containsAnyTokenMatching(string-join($bannerFGIsourceOpen_tok,' '), ('^NATO:?'))" flag="error" role="error">
            [ISM-ID-00316][Error] If ISM_USGOV_RESOURCE and attribute @ism:declassException of ISM_RESOURCE_ELEMENT contains 
            [NATO] then at least one element meeting ISM_CONTRIBUTES in the document must have a 
            @ism:ownerProducer attribute containing [NATO] or the resource level attribute @ism:FGIsourceOpen must contain [NATO].
            
            Human Readable: USA documents marked with a NATO declass exemption must have NATO portions or FGI NATO at the resource level.
        </sch:assert>
    </sch:rule>
</sch:pattern>